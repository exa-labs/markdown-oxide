use std::path::{Path, PathBuf};

use rayon::prelude::*;
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Url};

use crate::{
    config::Settings,
    vault::{Reference, Referenceable, Vault},
};

pub fn path_unresolved_references<'a>(
    vault: &'a Vault,
    path: &'a Path,
) -> Option<Vec<(&'a Path, &'a Reference)>> {
    let referenceables = vault.select_referenceable_nodes(None);
    let pathreferences = vault.select_references(Some(path))?;

    let unresolved = pathreferences
        .into_par_iter()
        .filter(|(path, reference)| {
            // Links that point outside the vault are handled by the out-of-vault
            // diagnostic, not as unresolved references.
            if reference.is_outside_vault(vault.root_dir()) {
                return false;
            }

            let matched_option = referenceables
                .iter()
                .find(|referenceable| reference.references(vault.root_dir(), path, referenceable));

            matched_option.is_some_and(|matched| {
                matches!(
                    matched,
                    Referenceable::UnresovledIndexedBlock(..)
                        | Referenceable::UnresovledFile(..)
                        | Referenceable::UnresolvedHeading(..)
                )
            })
        })
        .collect::<Vec<_>>();

    Some(unresolved)
}

fn out_of_vault_references<'a>(
    vault: &'a Vault,
    path: &'a Path,
) -> Option<Vec<(&'a Path, &'a Reference)>> {
    let pathreferences = vault.select_references(Some(path))?;

    Some(
        pathreferences
            .into_par_iter()
            .filter(|(_, reference)| reference.is_outside_vault(vault.root_dir()))
            .collect::<Vec<_>>(),
    )
}

fn diagnostic_message(reference: &Reference, count: usize) -> String {
    if reference.is_attachment() {
        match count {
            1 => "Attachment outside vault".to_string(),
            n => format!("Attachment outside vault used {} times", n),
        }
    } else {
        match count {
            1 => "Link target outside vault".to_string(),
            n => format!("Link target outside vault used {} times", n),
        }
    }
}

pub fn diagnostics(
    vault: &Vault,
    settings: &Settings,
    (path, _uri): (&PathBuf, &Url),
) -> Option<Vec<Diagnostic>> {
    if !settings.unresolved_diagnostics {
        return None;
    }

    let allreferences = vault.select_references(None)?;

    let unresolved = path_unresolved_references(vault, path)?;
    let outside = out_of_vault_references(vault, path)?;

    let mut diags: Vec<Diagnostic> = unresolved
        .into_par_iter()
        .map(|(_, reference)| {
            let count = allreferences
                .iter()
                .filter(|(other_path, otherreference)| {
                    otherreference.matches_type(reference)
                        && (!matches!(reference, Reference::Footnote(_)) || **other_path == *path)
                        && otherreference.data().reference_text == reference.data().reference_text
                })
                .count();

            Diagnostic {
                range: *reference.data().range,
                message: match count {
                    1 => "Unresolved Reference".to_string(),
                    n => format!("Unresolved Reference used {} times", n),
                },
                source: Some("markdown-oxide".into()),
                severity: Some(DiagnosticSeverity::INFORMATION),
                ..Default::default()
            }
        })
        .collect();

    let outside_diags: Vec<Diagnostic> = outside
        .into_par_iter()
        .map(|(_, reference)| {
            let count = allreferences
                .iter()
                .filter(|(_, otherreference)| {
                    otherreference.matches_type(reference)
                        && otherreference.data().reference_text == reference.data().reference_text
                })
                .count();

            Diagnostic {
                range: *reference.data().range,
                message: diagnostic_message(reference, count),
                source: Some("markdown-oxide".into()),
                severity: Some(DiagnosticSeverity::WARNING),
                ..Default::default()
            }
        })
        .collect();

    diags.extend(outside_diags);

    Some(diags)
}
