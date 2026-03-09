use crate::model::normalized_records::{
    NormalizedVoteRecord, ProceduralKind, VoteCategory, VotePosition,
};

pub fn cloture_votes<'a>(votes: &[&'a NormalizedVoteRecord]) -> Vec<&'a NormalizedVoteRecord> {
    votes.iter()
        .copied()
        .filter(|vote| {
            vote.vote_category == VoteCategory::Cloture
                || vote.procedural_kind == Some(ProceduralKind::Cloture)
        })
        .collect()
}

pub fn motion_to_proceed_votes<'a>(
    votes: &[&'a NormalizedVoteRecord],
) -> Vec<&'a NormalizedVoteRecord> {
    votes.iter()
        .copied()
        .filter(|vote| {
            vote.vote_category == VoteCategory::MotionToProceed
                || vote.procedural_kind == Some(ProceduralKind::MotionToProceed)
        })
        .collect()
}

pub fn amendment_votes<'a>(votes: &[&'a NormalizedVoteRecord]) -> Vec<&'a NormalizedVoteRecord> {
    votes.iter()
        .copied()
        .filter(|vote| {
            vote.vote_category == VoteCategory::Amendment
                || vote.procedural_kind == Some(ProceduralKind::AmendmentProcess)
        })
        .collect()
}

pub fn support_rate(votes: &[&NormalizedVoteRecord]) -> Option<f32> {
    let mut support = 0usize;
    let mut total = 0usize;
    for vote in votes {
        match vote.vote_position {
            VotePosition::Yea => {
                support += 1;
                total += 1;
            }
            VotePosition::Nay => total += 1,
            _ => {}
        }
    }
    if total == 0 {
        None
    } else {
        Some(support as f32 / total as f32)
    }
}
