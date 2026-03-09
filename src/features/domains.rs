use crate::model::{
    identity::Party,
    legislative::PolicyDomain,
    normalized_records::{NormalizedVoteRecord, VotePosition},
};

pub fn domain_score(
    votes: &[&NormalizedVoteRecord],
    domain: &PolicyDomain,
    party: &Party,
) -> (f32, usize) {
    let relevant = votes
        .iter()
        .copied()
        .filter(|vote| vote.policy_domain.as_ref() == Some(domain))
        .collect::<Vec<_>>();

    if relevant.is_empty() {
        return (fallback_domain_score(domain, party), 0);
    }

    let mut sum = 0.0f32;
    let mut total = 0usize;
    for vote in relevant {
        match vote.vote_position {
            VotePosition::Yea => {
                sum += 1.0;
                total += 1;
            }
            VotePosition::Nay => {
                sum -= 1.0;
                total += 1;
            }
            _ => {}
        }
    }
    if total == 0 {
        (fallback_domain_score(domain, party), 0)
    } else {
        ((sum / total as f32).clamp(-1.0, 1.0), total)
    }
}

pub fn fallback_domain_score(domain: &PolicyDomain, party: &Party) -> f32 {
    match party {
        Party::Democrat => match domain {
            PolicyDomain::Defense => 0.10,
            PolicyDomain::BudgetTax => -0.15,
            PolicyDomain::Healthcare => 0.30,
            PolicyDomain::Immigration => 0.20,
            PolicyDomain::EnergyClimate => 0.35,
            PolicyDomain::Judiciary => -0.05,
            PolicyDomain::Technology => 0.15,
            PolicyDomain::ForeignPolicy => 0.10,
            PolicyDomain::Labor => 0.30,
            PolicyDomain::Education => 0.25,
            PolicyDomain::Other(_) => 0.0,
        },
        Party::Republican => match domain {
            PolicyDomain::Defense => 0.25,
            PolicyDomain::BudgetTax => 0.25,
            PolicyDomain::Healthcare => -0.20,
            PolicyDomain::Immigration => -0.10,
            PolicyDomain::EnergyClimate => -0.30,
            PolicyDomain::Judiciary => 0.20,
            PolicyDomain::Technology => -0.05,
            PolicyDomain::ForeignPolicy => 0.20,
            PolicyDomain::Labor => -0.20,
            PolicyDomain::Education => -0.10,
            PolicyDomain::Other(_) => 0.0,
        },
        _ => 0.0,
    }
}
