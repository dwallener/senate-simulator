use std::collections::BTreeMap;

use crate::{
    model::{
        identity::Party,
        normalized_records::{NormalizedVoteRecord, VotePosition},
    },
};

pub fn group_votes_by_roll_call<'a>(
    votes: &[&'a NormalizedVoteRecord],
) -> BTreeMap<String, Vec<&'a NormalizedVoteRecord>> {
    let mut grouped: BTreeMap<String, Vec<&'a NormalizedVoteRecord>> = BTreeMap::new();
    for vote in votes {
        grouped.entry(vote.vote_id.clone()).or_default().push(*vote);
    }
    grouped
}

pub fn party_majority_position(
    votes: &[&NormalizedVoteRecord],
    party: &Party,
) -> Option<VotePosition> {
    let mut yea = 0usize;
    let mut nay = 0usize;
    for vote in votes.iter().filter(|vote| &vote.party_at_time == party) {
        match vote.vote_position {
            VotePosition::Yea => yea += 1,
            VotePosition::Nay => nay += 1,
            _ => {}
        }
    }

    match yea.cmp(&nay) {
        std::cmp::Ordering::Greater => Some(VotePosition::Yea),
        std::cmp::Ordering::Less => Some(VotePosition::Nay),
        std::cmp::Ordering::Equal => None,
    }
}

pub fn valid_participation(vote: &NormalizedVoteRecord) -> bool {
    matches!(
        vote.vote_position,
        VotePosition::Yea | VotePosition::Nay | VotePosition::Present
    )
}

pub fn is_support_position(vote: &NormalizedVoteRecord) -> Option<bool> {
    match vote.vote_position {
        VotePosition::Yea => Some(true),
        VotePosition::Nay => Some(false),
        _ => None,
    }
}
