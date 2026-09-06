use super::*;

fn fields(base: &str, head: &str) -> Vec<String> {
    [
        "occupancy-freshness-v1",
        "owner/repo",
        "owner/repo",
        "owner/repo",
        base,
        head,
    ]
    .into_iter()
    .map(str::to_owned)
    .chain(['a', 'a', 'b', 'c', 'd', 'd'].map(|c| c.to_string().repeat(40)))
    .collect()
}

fn parse(fields: &[String]) -> Result<FreshnessProof, String> {
    FreshnessProof::parse(&fields.join("\n"))
}

#[test]
fn all_closed_rungs_distinguish_ancestry_equivalence_and_stale_trees() {
    for (base, head) in [
        ("staging", "dev"),
        ("canary", "staging"),
        ("production", "canary"),
    ] {
        let mut f = fields(base, head);
        assert_eq!(
            parse(&f).unwrap().kind(),
            HubBaseFreshness::EquivalentPromotionTree
        );
        f[11] = "e".repeat(40);
        assert_eq!(parse(&f).unwrap().kind(), HubBaseFreshness::Stale);
        f[9] = f[8].clone();
        assert!(
            parse(&f).is_err(),
            "one commit cannot have contradictory trees"
        );
        f[11] = f[10].clone();
        assert_eq!(
            parse(&f).unwrap().kind(),
            HubBaseFreshness::AtDestinationTip
        );
    }
}

#[test]
fn ordinary_and_fork_features_require_literal_destination_tip_ancestry() {
    assert!(parse(&fields("dev", " ")).is_err(), "blank feature ref");
    for base in ["dev", "main"] {
        for repo in ["owner/repo", "fork/repo"] {
            let mut f = fields(base, "feature/topic");
            f[3] = repo.to_owned();
            assert_eq!(parse(&f).unwrap().kind(), HubBaseFreshness::Stale);
            f[9] = f[8].clone();
            assert_eq!(
                parse(&f).unwrap().kind(),
                HubBaseFreshness::AtDestinationTip
            );
        }
    }
}

#[test]
fn repository_identity_and_closed_ladder_are_required_even_at_tip() {
    for (index, bad) in [
        (1, ""),
        (2, "elsewhere/repo"),
        (3, "fork/repo"),
        (3, ""),
        (3, "owner/repo/extra"),
        (3, "owner /repo"),
        (4, "unknown"),
        (4, "production"),
        (5, "feature"),
        (5, ""),
    ] {
        let mut f = fields("staging", "dev");
        f[9] = f[8].clone();
        f[index] = bad.to_owned();
        assert!(parse(&f).is_err(), "field {index}");
    }
}

#[test]
fn records_are_exact_and_never_trimmed_into_validity() {
    let f = fields("staging", "dev");
    let record = f.join("\n");
    assert!(FreshnessProof::parse(&format!("{record}\n")).is_ok());
    for invalid in [
        format!("{record}\n\n"),
        format!("{record}\nextra"),
        format!(" {record}"),
        record.replace('\n', "\r\n"),
        f[..11].join("\n"),
    ] {
        assert!(FreshnessProof::parse(&invalid).is_err());
    }
    for (index, bad) in [
        (0, "v2"),
        (5, "feature\tbranch"),
        (9, ""),
        (9, "abc"),
        (9, "gggggggggggggggggggggggggggggggggggggggg"),
        (9, "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC"),
        (
            9,
            "cccccccccccccccccccccccccccccccccccccccc\ncccccccccccccccccccccccccccccccccccccccc",
        ),
    ] {
        let mut changed = f.clone();
        changed[index] = bad.to_owned();
        assert!(parse(&changed).is_err(), "field {index}");
    }
}

#[test]
fn object_formats_are_full_consistent_and_bound_to_event_head() {
    for width in [40, 64] {
        let mut f = fields("staging", "dev");
        for id in &mut f[6..] {
            *id = id[..1].repeat(width);
        }
        assert!(parse(&f).is_ok());
        f[7] = "e".repeat(width);
        assert!(parse(&f).is_err());
        f[7] = f[6].clone();
        f[11] = "d".repeat(if width == 40 { 64 } else { 40 });
        assert!(parse(&f).is_err());
    }
}

#[test]
fn long_display_is_bounded_without_truncating_compared_identity() {
    let mut f = fields("dev", &"界".repeat(200));
    let identity = format!("owner/{}", "x".repeat(200));
    f[1] = identity.clone();
    f[2] = identity;
    let proof = parse(&f).unwrap();
    let diagnostic = proof.diagnostic();
    assert!(diagnostic.contains("…[truncated]"));
    assert!(!diagnostic.contains(&"界".repeat(129)));
    assert!(diagnostic.contains(&f[8]));
    assert!(diagnostic.contains("Stale"));
    f[2].push('x');
    assert!(
        parse(&f).is_err(),
        "display-prefix equality is not identity equality"
    );
}

#[test]
fn actual_reader_and_parser_bound_records_and_fields_with_fixed_errors() {
    let record = fields("staging", "dev").join("\n");
    assert!(read_record(record.as_bytes()).is_ok());
    let oversized = vec![b'x'; MAX_RECORD_BYTES + 1];
    let error = read_record(oversized.as_slice()).unwrap_err();
    assert!(error.len() < 128);
    assert!(FreshnessProof::parse(std::str::from_utf8(&oversized).unwrap()).is_err());
    let mut f = fields("dev", "feature");
    f[5] = "x".repeat(MAX_FIELD_BYTES + 1);
    assert!(parse(&f).unwrap_err().len() < 128);
    assert!(read_record(&[0xff_u8][..]).unwrap_err().len() < 128);
}
