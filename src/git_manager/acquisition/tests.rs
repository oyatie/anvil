use super::*;
use std::sync::Mutex;

fn identity() -> RepoIdentity {
    RepoIdentity::parse("first/shared").unwrap()
}
fn base() -> PathBuf {
    PathBuf::from("/ordinary/repos")
}

fn observation() -> Observation {
    let target = identity().path(&base());
    Observation {
        base: base(),
        top: target.clone(),
        common: target.join(".git"),
        target,
        primary: true,
        fetch: identity().clone_url().into_bytes(),
        push: identity().clone_url().into_bytes(),
        helper_override: false,
    }
}

#[test]
fn every_observed_identity_dimension_is_required() {
    let changes: [fn(&mut Observation); 8] = [
        |o| o.primary = false,
        |o| o.target = base().join("wrong"),
        |o| o.top = base(),
        |o| o.common = base().join("other.git"),
        |o| o.fetch = b"https://github.com/wrong/shared.git".to_vec(),
        |o| o.push = b"https://github.com/wrong/shared.git".to_vec(),
        |o| o.helper_override = true,
        |o| o.base = base().join("other"),
    ];
    validate(&identity(), &observation()).unwrap();
    for change in changes {
        let mut observed = observation();
        change(&mut observed);
        assert!(validate(&identity(), &observed).is_err());
    }
}

struct Fake {
    location: Location,
    fail: &'static str,
    change: fn(&mut Observation),
    events: Mutex<Vec<&'static str>>,
}
impl Fake {
    fn new(location: Location) -> Self {
        Self {
            location,
            fail: "",
            change: |_| {},
            events: Mutex::new(Vec::new()),
        }
    }
    fn event(&self, name: &'static str) -> Result<()> {
        self.events.lock().unwrap().push(name);
        if self.fail == name {
            bail!("inert scripted observation error");
        }
        Ok(())
    }
    fn events(&self) -> Vec<&'static str> {
        self.events.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl CheckoutIo for Fake {
    async fn location(&self, _: &Path, _: &RepoIdentity) -> Result<Location> {
        self.event("location")?;
        Ok(self.location)
    }
    async fn prepare(&self, _: &Path) -> Result<()> {
        self.event("prepare")
    }
    async fn clone_repo(&self, _: &RepoIdentity, _: &Path) -> Result<()> {
        self.event("clone")
    }
    async fn observe(&self, _: &Path, _: &Path) -> Result<Observation> {
        self.event("observe")?;
        let mut observed = observation();
        (self.change)(&mut observed);
        Ok(observed)
    }
    async fn refresh(&self, _: &Path) {
        let _ = self.event("refresh");
    }
    async fn hooks(&self, _: &Path) {
        let _ = self.event("hooks");
    }
}

#[tokio::test]
async fn valid_existing_checkout_is_observed_before_any_mutation() {
    let io = Fake::new(Location::Existing);
    assert_eq!(
        acquire_with(&io, &base(), &identity()).await.unwrap(),
        identity().path(&base())
    );
    assert_eq!(
        io.events(),
        ["location", "observe", "prepare", "refresh", "hooks"]
    );
}

#[tokio::test]
async fn new_clone_requires_observed_identity_before_hooks() {
    let io = Fake::new(Location::Vacant);
    acquire_with(&io, &base(), &identity()).await.unwrap();
    assert_eq!(
        io.events(),
        ["location", "prepare", "clone", "observe", "hooks"]
    );
}

#[tokio::test]
async fn legacy_and_failed_observations_do_not_mutate_or_fall_back() {
    let io = Fake::new(Location::Legacy);
    assert!(
        acquire_with(&io, &base(), &identity())
            .await
            .unwrap_err()
            .to_string()
            .contains("MigrationRequired")
    );
    assert_eq!(io.events(), ["location"]);
    for fail in ["location", "observe"] {
        let mut io = Fake::new(Location::Existing);
        io.fail = fail;
        assert!(acquire_with(&io, &base(), &identity()).await.is_err());
        assert!(
            io.events()
                .iter()
                .all(|event| ["location", "observe"].contains(event))
        );
    }
}

#[tokio::test]
async fn invalid_existing_identity_stops_before_mutation() {
    for change in [
        (|o: &mut Observation| o.fetch.clear()) as fn(&mut Observation),
        |o| o.push.clear(),
        |o| o.primary = false,
        |o| o.helper_override = true,
    ] {
        let mut io = Fake::new(Location::Existing);
        io.change = change;
        assert!(acquire_with(&io, &base(), &identity()).await.is_err());
        assert_eq!(io.events(), ["location", "observe"]);
    }
}

#[tokio::test]
async fn clone_failures_leave_partial_state_unmodified() {
    for fail in ["prepare", "clone", "observe"] {
        let mut io = Fake::new(Location::Vacant);
        io.fail = fail;
        assert!(acquire_with(&io, &base(), &identity()).await.is_err());
        assert!(!io.events().contains(&"hooks"));
        assert!(!io.events().contains(&"refresh"));
    }
    let mut io = Fake::new(Location::Vacant);
    io.change = |o| o.push.clear();
    assert!(acquire_with(&io, &base(), &identity()).await.is_err());
    assert_eq!(io.events(), ["location", "prepare", "clone", "observe"]);
}

#[tokio::test]
async fn best_effort_refresh_and_hooks_do_not_claim_success_evidence() {
    for fail in ["refresh", "hooks"] {
        let mut io = Fake::new(Location::Existing);
        io.fail = fail;
        acquire_with(&io, &base(), &identity()).await.unwrap();
        assert_eq!(
            io.events(),
            ["location", "observe", "prepare", "refresh", "hooks"]
        );
    }
}

#[test]
fn actual_acquisition_and_gc_keep_the_private_validation_boundary() {
    let source = crate::source_scan::paths::module_source(
        "src/git_manager",
        Path::new(env!("CARGO_MANIFEST_DIR")),
    );
    let compact = crate::source_scan::without_commentary(&source)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>();
    assert_eq!(compact.matches("SubjectRoot::cloned(").count(), 1);
    assert!(compact.contains("letrepo_dir=acquisition::acquire(&self.repos_base_dir,repo).await?;Ok(SubjectRoot::cloned(repo_dir))"));
    assert!(
        compact.contains(
            "acquisition::validate_existing(&self.repos_base_dir,&identity).await.is_ok()"
        )
    );
}
