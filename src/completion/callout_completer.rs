use once_cell::sync::Lazy;
use regex::Regex;
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionItemLabelDetails, CompletionTextEdit,
    InsertTextFormat, Position, Range, TextEdit,
};

use super::{Completable, Completer};

pub struct CalloutCompleter {
    nested_level: usize,
    line: u32,
    character: u32,
    preceding_text: String,
}

/// Decide whether the callout completer should fire given the text on the line up to the cursor.
///
/// Returns `(preceding_text, nested_level)` only when the cursor sits where an Obsidian callout
/// could be typed: directly after the `(> *)+` block-quote prefix, with at most a partial callout
/// opener (`[`, `[!`, `[!type`) or a bare type word before it. Any other content — a space, an
/// apostrophe, a finished word — means the user is writing prose, so no completion is offered.
fn callout_trigger(line_to_cursor: &str) -> Option<(String, usize)> {
    static PARTIAL_CALLOUT: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^(?<preceding>(> *)+)\[?!?[\w-]*$").unwrap());

    let captures = PARTIAL_CALLOUT.captures(line_to_cursor)?;
    let preceding = captures.name("preceding")?.as_str();
    let nested_level = preceding.matches('>').count();

    Some((preceding.to_string(), nested_level))
}

impl<'a> Completer<'a> for CalloutCompleter {
    fn construct(context: super::Context<'a>, line: usize, character: usize) -> Option<Self>
    where
        Self: Sized + Completer<'a>,
    {
        if !context.settings.callout_completions {
            return None;
        }

        let line_chars = context.vault.select_line(context.path, line as isize)?;
        let line_to_cursor = String::from_iter(line_chars.get(0..character)?);

        let (preceding_text, nested_level) = callout_trigger(&line_to_cursor)?;

        Some(Self {
            nested_level,
            preceding_text,
            line: line as u32,
            character: character as u32,
        })
    }

    fn completions(&self) -> Vec<impl super::Completable<'a, Self>>
    where
        Self: Sized,
    {
        vec![
            CalloutCompletion::Note,
            CalloutCompletion::Abstract,
            CalloutCompletion::Summary,
            CalloutCompletion::Tldr,
            CalloutCompletion::Info,
            CalloutCompletion::Todo,
            CalloutCompletion::Tip,
            CalloutCompletion::Hint,
            CalloutCompletion::Important,
            CalloutCompletion::Success,
            CalloutCompletion::Check,
            CalloutCompletion::Done,
            CalloutCompletion::Question,
            CalloutCompletion::Help,
            CalloutCompletion::Faq,
            CalloutCompletion::Warning,
            CalloutCompletion::Caution,
            CalloutCompletion::Attention,
            CalloutCompletion::Failure,
            CalloutCompletion::Fail,
            CalloutCompletion::Missing,
            CalloutCompletion::Danger,
            CalloutCompletion::Error,
            CalloutCompletion::Bug,
            CalloutCompletion::Example,
            CalloutCompletion::Quote,
            CalloutCompletion::Cite,
        ]
    }

    // TODO: get rid of this in the API
    type FilterParams = &'static str;
    fn completion_filter_text(&self, params: Self::FilterParams) -> String {
        format!("{}{}", self.preceding_text, params)
    }
}

enum CalloutCompletion {
    Note,
    Abstract,
    Summary,
    Tldr,
    Info,
    Todo,
    Tip,
    Hint,
    Important,
    Success,
    Check,
    Done,
    Question,
    Help,
    Faq,
    Warning,
    Caution,
    Attention,
    Failure,
    Fail,
    Missing,
    Danger,
    Error,
    Bug,
    Example,
    Quote,
    Cite,
}

impl Completable<'_, CalloutCompleter> for CalloutCompletion {
    fn completions(&self, completer: &CalloutCompleter) -> Option<CompletionItem> {
        let name = match self {
            Self::Note => "note",
            Self::Abstract => "abstract",
            Self::Summary => "summary",
            Self::Tldr => "tldr",
            Self::Info => "info",
            Self::Todo => "todo",
            Self::Tip => "tip",
            Self::Hint => "hint",
            Self::Important => "important",
            Self::Success => "success",
            Self::Check => "check",
            Self::Done => "done",
            Self::Question => "question",
            Self::Help => "help",
            Self::Faq => "faq",
            Self::Warning => "warning",
            Self::Caution => "caution",
            Self::Attention => "attention",
            Self::Failure => "failure",
            Self::Fail => "fail",
            Self::Missing => "missing",
            Self::Danger => "danger",
            Self::Error => "error",
            Self::Bug => "bug",
            Self::Example => "example",
            Self::Quote => "quote",
            Self::Cite => "cite",
        };

        let label_detail = match self {
            Self::Summary | Self::Tldr => Some("alias of Abstract"),
            Self::Hint | Self::Important => Some("alias of Tip"),
            Self::Check | Self::Done => Some("alias of Success"),
            Self::Help | Self::Faq => Some("alias of Question"),
            Self::Caution | Self::Attention => Some("alias of Warning"),
            Self::Fail | Self::Missing => Some("alias of Failure"),
            Self::Error => Some("alias of Danger"),
            Self::Cite => Some("alias of Quote"),
            _ => None,
        };

        let snippet = format!(
            "{prefix}[!{name}] ${{1:Title}}\n{prefix}${{2:Description}}",
            prefix = "> ".repeat(completer.nested_level)
        );

        let filter_text = completer.completion_filter_text(name);

        let completion_item = CompletionItem {
            label: name.to_string(),
            label_details: label_detail.map(|detail| CompletionItemLabelDetails {
                detail: Some(detail.to_string()),
                description: None,
            }),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            kind: Some(CompletionItemKind::SNIPPET),
            text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                range: Range {
                    start: Position {
                        line: completer.line,
                        character: 0,
                    },
                    end: Position {
                        line: completer.line,
                        character: completer.character,
                    },
                },
                new_text: snippet,
            })),
            filter_text: Some(filter_text),
            ..Default::default()
        };

        Some(completion_item)
    }
}

#[cfg(test)]
mod tests {
    use super::callout_trigger;

    fn nested(line_to_cursor: &str) -> Option<usize> {
        callout_trigger(line_to_cursor).map(|(_, level)| level)
    }

    #[test]
    fn fires_at_blockquote_marker() {
        // Empty content directly after the prefix.
        assert_eq!(nested("> "), Some(1));
        assert_eq!(nested(">"), Some(1));
        assert_eq!(nested(">>"), Some(2));
        assert_eq!(nested(">> "), Some(2));
    }

    #[test]
    fn fires_while_typing_callout_opener() {
        assert_eq!(nested("> ["), Some(1));
        assert_eq!(nested("> [!"), Some(1));
        assert_eq!(nested("> [!no"), Some(1));
        assert_eq!(nested("> [!note"), Some(1));
        // Bare type word (the completer also offers the snippet for `> note`).
        assert_eq!(nested("> no"), Some(1));
        assert_eq!(nested(">> [!war"), Some(2));
    }

    #[test]
    fn suppressed_once_prose_is_written() {
        // The reported bug: a finished word followed by more prose must not re-trigger.
        assert_eq!(callout_trigger("> We're going to have to decide"), None);
        // A single word followed by a space (space is a completion trigger char).
        assert_eq!(callout_trigger("> note "), None);
        assert_eq!(callout_trigger("> hello world"), None);
        // Apostrophe / punctuation is not a callout-opener character.
        assert_eq!(callout_trigger("> We're"), None);
        // A closed callout opener is past the type-selection point.
        assert_eq!(callout_trigger("> [!note]"), None);
        assert_eq!(callout_trigger("> [!note] Title"), None);
    }

    #[test]
    fn requires_blockquote_prefix() {
        assert_eq!(callout_trigger(""), None);
        assert_eq!(callout_trigger("note"), None);
        assert_eq!(callout_trigger("[!note]"), None);
    }
}
