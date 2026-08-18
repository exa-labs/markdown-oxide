use std::ops::Deref;

use nucleo_matcher::{
    pattern::{self, Normalization},
    Matcher,
};
use tower_lsp::lsp_types::CompletionItem;

use crate::config::Case;

use super::{Completable, Completer};

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CompletionPriority {
    Top,
    Elevated,
    Normal,
}

pub trait Matchable {
    fn match_string(&self) -> &str;

    fn priority(&self) -> CompletionPriority {
        CompletionPriority::Normal
    }
}

struct NucleoMatchable<T: Matchable>(T);
impl<T: Matchable> Deref for NucleoMatchable<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: Matchable> AsRef<str> for NucleoMatchable<T> {
    fn as_ref(&self) -> &str {
        self.match_string()
    }
}

pub struct OrderedCompletion<'a, C, T>
where
    C: Completer<'a>,
    T: Completable<'a, C>,
{
    completable: T,
    rank: String,
    __phantom: std::marker::PhantomData<&'a T>,
    __phantom2: std::marker::PhantomData<C>,
}

impl<'a, C: Completer<'a>, T: Completable<'a, C>> OrderedCompletion<'a, C, T> {
    pub fn new(completable: T, rank: String) -> Self {
        Self {
            completable,
            rank,
            __phantom: std::marker::PhantomData,
            __phantom2: std::marker::PhantomData,
        }
    }
}

impl<'a, C: Completer<'a>, T: Completable<'a, C>> Completable<'a, C>
    for OrderedCompletion<'a, C, T>
{
    fn completions(&self, completer: &C) -> Option<CompletionItem> {
        let completion = self.completable.completions(completer);

        completion.map(|completion| CompletionItem {
            sort_text: Some(self.rank.to_string()),
            ..completion
        })
    }
}

pub fn fuzzy_match_completions<'a, 'b, C: Completer<'a>, T: Matchable + Completable<'a, C>>(
    filter_text: &'b str,
    items: impl IntoIterator<Item = T>,
    case: &Case,
) -> Vec<OrderedCompletion<'a, C, T>> {
    let fuzzy_matches = order_matches(fuzzy_match(filter_text, items, case));

    fuzzy_matches
        .into_iter()
        .map(|(item, score)| {
            let priority = item.priority();
            OrderedCompletion::new(item, format_sort_text(priority, score))
        })
        .collect::<Vec<_>>()
}

fn order_matches<T: Matchable>(mut fuzzy_matches: Vec<(T, u32)>) -> Vec<(T, u32)> {
    fuzzy_matches.sort_by(|(left, left_score), (right, right_score)| {
        left.priority()
            .cmp(&right.priority())
            .then_with(|| right_score.cmp(left_score))
    });
    fuzzy_matches
}

fn format_sort_text(priority: CompletionPriority, score: u32) -> String {
    format!("{}{:010}", priority as u8, u32::MAX - score)
}

pub fn fuzzy_match<T: Matchable>(
    filter_text: &str,
    items: impl IntoIterator<Item = T>,
    case: &Case,
) -> Vec<(T, u32)> {
    let items = items.into_iter().map(NucleoMatchable);

    let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);
    let matches = pattern::Pattern::parse(
        filter_text,
        match case {
            Case::Smart => pattern::CaseMatching::Smart,
            Case::Ignore => pattern::CaseMatching::Ignore,
            Case::Respect => pattern::CaseMatching::Respect,
        },
        Normalization::Smart,
    )
    .match_list(items, &mut matcher);

    matches
        .into_iter()
        .map(|(item, score)| (item.0, score))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{format_sort_text, fuzzy_match, order_matches, CompletionPriority, Matchable};
    use crate::config::Case;

    struct TestMatchable {
        match_string: &'static str,
        priority: CompletionPriority,
    }

    impl Matchable for TestMatchable {
        fn match_string(&self) -> &str {
            self.match_string
        }

        fn priority(&self) -> CompletionPriority {
            self.priority
        }
    }

    #[test]
    fn priority_is_applied_before_fuzzy_score() {
        let matches = fuzzy_match(
            "today",
            [
                TestMatchable {
                    match_string: "t o d a y",
                    priority: CompletionPriority::Top,
                },
                TestMatchable {
                    match_string: "today",
                    priority: CompletionPriority::Normal,
                },
            ],
            &Case::Smart,
        );

        let top_score = matches
            .iter()
            .find(|(item, _)| item.priority() == CompletionPriority::Top)
            .unwrap()
            .1;
        let normal_score = matches
            .iter()
            .find(|(item, _)| item.priority() == CompletionPriority::Normal)
            .unwrap()
            .1;
        assert!(top_score < normal_score);

        let ordered = order_matches(matches);
        assert_eq!(ordered[0].0.priority(), CompletionPriority::Top);
    }

    #[test]
    fn sort_text_is_lexicographically_ordered_by_priority_and_score() {
        let top_lower_score = format_sort_text(CompletionPriority::Top, 100);
        let normal_higher_score = format_sort_text(CompletionPriority::Normal, 200);
        let top_higher_score = format_sort_text(CompletionPriority::Top, 200);
        let normal_score_21 = format_sort_text(CompletionPriority::Normal, 21);
        let normal_score_2 = format_sort_text(CompletionPriority::Normal, 2);

        assert!(top_lower_score < normal_higher_score);
        assert!(top_higher_score < top_lower_score);
        assert!(normal_score_21 < normal_score_2);
    }
}
