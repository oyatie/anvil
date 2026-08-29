//! The `anvil shape` subcommand tree.
//!
//! Split from `handlers.rs` to hold the dispatcher inside D-35's budget. These
//! are the arms that reach neither `AppState` nor the network.

use anyhow::Result;

use crate::cli::args::ShapeAction;
use crate::cli::opt_read::read_opt;

pub(super) async fn dispatch(action: ShapeAction) -> Result<()> {
    match action {
        crate::cli::args::ShapeAction::ValidateSpec { path, registry } => {
            let summary =
                crate::shape::facade::cli::validate_spec_file(&path, registry.as_deref())?;
            println!("{}", summary.render());
        }
        crate::cli::args::ShapeAction::Measure {
            repo_dir,
            rev,
            repo,
            spec_override,
            registry,
            json,
        } => {
            let label = repo.unwrap_or_else(|| {
                repo_dir
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| repo_dir.display().to_string())
            });
            let req = crate::shape::facade::measure::MeasureRequest {
                repo_dir,
                rev,
                repo: label,
                spec_override,
                registry_override: registry,
            };
            let report = crate::shape::facade::measure::measure_repo(&req).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print!("{}", crate::shape::facade::measure::render(&report));
            }
        }
        crate::cli::args::ShapeAction::Admit {
            repo_dir,
            rev,
            repo,
        } => {
            let label = repo.unwrap_or_else(|| {
                repo_dir
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| repo_dir.display().to_string())
            });
            let report =
                crate::shape::facade::admit::admit(&crate::shape::facade::admit::AdmitRequest {
                    repo_dir,
                    rev,
                })
                .await?;
            println!("{}", crate::shape::facade::admit::render(&report, &label));
        }
        crate::cli::args::ShapeAction::Baseline {
            repo_dir,
            rev,
            spec_override,
            out,
        } => {
            // The document already on disk is the reference a regeneration
            // must shrink against. Reading it is what makes
            // `regen_is_monotonic` reachable: without it, `--out` simply
            // overwrites the committed baseline with whatever the tree
            // produces now, laundering every key that appeared in between.
            use crate::ratchet::facade::{Baseline, Signoff};
            let previous = read_opt(out.as_ref()).await.ok().flatten();
            let previous = previous.and_then(|b| Baseline::parse(&b).ok());
            let signoff =
                tokio::fs::read(repo_dir.join(crate::shape::facade::baseline::SIGNOFF_PATH))
                    .await
                    .ok()
                    .and_then(|b| Signoff::parse(&b).ok())
                    .unwrap_or_default();
            let (baseline, report) = crate::shape::facade::baseline::reseed_from_commit(
                &repo_dir,
                &rev,
                spec_override.as_deref(),
                previous.as_ref(),
                &signoff,
            )
            .await?;
            let json = baseline.to_json();
            match out {
                Some(p) => {
                    if let Some(parent) = p.parent() {
                        tokio::fs::create_dir_all(parent).await?;
                    }
                    tokio::fs::write(&p, format!("{json}\n")).await?;
                    println!(
                        "baseline written to {} ({} key(s) across {} rule(s), measured at {})",
                        p.display(),
                        baseline.total_keys(),
                        baseline.rules.len(),
                        &report.rev[..12]
                    );
                }
                None => println!("{json}"),
            }
        }
        crate::cli::args::ShapeAction::Plan {
            repo_dir,
            rev,
            spec_override,
            policy,
            plan_out,
        } => {
            let req = crate::shape::facade::measure::MeasureRequest {
                repo_dir: repo_dir.clone(),
                rev,
                repo: repo_dir
                    .canonicalize()
                    .ok()
                    .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                    .unwrap_or_default(),
                spec_override,
                registry_override: None,
            };
            let report = crate::shape::facade::measure::measure_repo(&req).await?;
            let spec_version = format!("{:?}", report.spec_source);
            let plan =
                crate::change_delivery::facade::plan::plan_from_report(&report, &spec_version);
            let owners =
                crate::change_delivery::facade::plan::owners_from_tree(&repo_dir, &report.rev)
                    .await;
            let manifests =
                crate::change_delivery::facade::plan::manifests_from_tree(&repo_dir, &report.rev)
                    .await;
            let policy_bytes = read_opt(policy.as_ref()).await?;
            let (policy, problem) =
                crate::change_delivery::facade::LandingPolicy::load(policy_bytes.as_deref());
            if let Some(p) = problem {
                println!("warning: {p}");
            }
            let d =
                crate::change_delivery::facade::plan::dry_run(&plan, &owners, &manifests, policy);
            print!(
                "{}",
                crate::change_delivery::facade::plan::render(&d, &plan)
            );
            if let Some(p) = plan_out {
                tokio::fs::write(&p, format!("{}\n", plan.to_json())).await?;
                println!("move plan written to {}", p.display());
            }
        }
        crate::cli::args::ShapeAction::Deliver {
            repo_dir,
            max,
            spec_override,
            allow_same_repo,
        } => {
            let label = repo_dir
                .canonicalize()
                .ok()
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                .unwrap_or_default();
            let req = crate::change_delivery::facade::deliver::DeliverRequest {
                repo_dir,
                repo: label,
                max,
                spec_override,
                allow_same_repo,
            };
            let (runs, shards, policy) =
                crate::change_delivery::facade::deliver::deliver_dry_run(&req).await?;
            print!(
                "{}",
                crate::change_delivery::facade::deliver::render(&runs, shards.len(), &policy)
            );
        }
        crate::cli::args::ShapeAction::Ratchet {
            repo_dir,
            base_ref,
            head,
            spec_override,
        } => {
            let j = crate::shape::facade::baseline::judge(
                &repo_dir,
                &base_ref,
                &head,
                spec_override.as_deref(),
            )
            .await?;
            print!("{}", crate::shape::facade::baseline::render_judgement(&j));
            if let crate::shape::facade::baseline::Judgement::Judged { verdict, .. } = &j
                && verdict.fails
            {
                anyhow::bail!("ratchet regressions present");
            }
        }
    }
    Ok(())
}
