use super::super::query::EvaluationQuery;
use crate::api::arith::AstPoint;
use crate::api::arith::AstScalar;
use crate::api::transcript::AstTranscript;
use crate::api::transcript::AstTranscriptReader;
use halo2_proofs::arithmetic::CurveAffine;
use halo2_proofs::plonk::VerifyingKey;
use halo2_proofs::poly::Rotation;
use std::iter;
use std::rc::Rc;

#[derive(Debug)]
pub struct CommonEvaluated<C: CurveAffine> {
    pub key: String,
    pub permutation_evals: Vec<Rc<AstScalar<C>>>,
    pub permutation_commitments: Vec<Rc<AstPoint<C>>>,
}

#[derive(Debug)]
pub struct EvaluatedSet<C: CurveAffine> {
    pub(crate) permutation_product_commitment: Rc<AstPoint<C>>,
    pub(crate) permutation_product_eval: Rc<AstScalar<C>>,
    pub(crate) permutation_product_next_eval: Rc<AstScalar<C>>,
    pub(crate) permutation_product_last_eval: Option<Rc<AstScalar<C>>>,
}

#[derive(Debug)]
pub struct Evaluated<C: CurveAffine> {
    pub(crate) key: String,
    pub(crate) blinding_factors: usize,
    pub(crate) x: Rc<AstScalar<C>>,
    pub(crate) sets: Vec<EvaluatedSet<C>>,
    pub(crate) evals: Vec<Rc<AstScalar<C>>>,
    pub(crate) chunk_len: usize,
}

impl<C: CurveAffine> Evaluated<C> {
    pub(crate) fn build_from_transcript(
        permutation_product_commitements: Vec<Rc<AstPoint<C>>>,
        key: &str,
        vk: &VerifyingKey<C>,
        transcript: &mut Rc<AstTranscript<C>>,
        x: &Rc<AstScalar<C>>,
        instance_evals: &Vec<Rc<AstScalar<C>>>,
        advice_evals: &Vec<Rc<AstScalar<C>>>,
        fixed_evals: &Vec<Rc<AstScalar<C>>>,
    ) -> Self {
        let n = permutation_product_commitements.len();

        let permutation_evaluated_set = permutation_product_commitements
            .into_iter()
            .enumerate()
            .map(|(i, permutation_product_commitment)| {
                let permutation_product_eval = transcript.read_scalar();
                let permutation_product_next_eval = transcript.read_scalar();
                let permutation_product_last_eval = if i < n {
                    Some(transcript.read_scalar())
                } else {
                    None
                };

                EvaluatedSet {
                    permutation_product_commitment,
                    permutation_product_eval,
                    permutation_product_next_eval,
                    permutation_product_last_eval,
                }
            })
            .collect();

        let permutation_evaluated_eval = vk
            .cs
            .permutation
            .columns
            .iter()
            .map(|column| match column.column_type() {
                halo2_proofs::plonk::Any::Advice => {
                    advice_evals[vk.cs.get_any_query_index(*column, Rotation::cur())].clone()
                }
                halo2_proofs::plonk::Any::Fixed => {
                    fixed_evals[vk.cs.get_any_query_index(*column, Rotation::cur())].clone()
                }
                halo2_proofs::plonk::Any::Instance => {
                    instance_evals[vk.cs.get_any_query_index(*column, Rotation::cur())].clone()
                }
            })
            .collect();

        Evaluated {
            x: x.clone(),
            blinding_factors: vk.cs.blinding_factors(),
            sets: permutation_evaluated_set,
            evals: permutation_evaluated_eval,
            chunk_len: vk.cs.degree() - 2,
            key: format!("{}_permutation", key.clone()),
        }
    }
}

impl<C: CurveAffine> Evaluated<C> {
    pub fn queries(
        &self,
        x_next: Rc<AstScalar<C>>,
        x_last: Rc<AstScalar<C>>,
    ) -> Vec<EvaluationQuery<C>> {
        iter::empty()
            .chain(self.sets.iter().enumerate().flat_map(|(i, set)| {
                iter::empty()
                    // Open permutation product commitments at x and \omega^{-1} x
                    // Open permutation product commitments at x and \omega x
                    .chain(Some(EvaluationQuery::new(
                        0,
                        self.x.clone(),
                        format!("{}_product_commitment_{}", self.key, i),
                        set.permutation_product_commitment.clone(),
                        set.permutation_product_eval.clone(),
                    )))
                    .chain(Some(EvaluationQuery::new(
                        1,
                        x_next.clone(),
                        format!("{}_product_commitment_{}", self.key, i),
                        set.permutation_product_commitment.clone(),
                        set.permutation_product_next_eval.clone(),
                    )))
            }))
            // Open it at \omega^{last} x for all but the last set
            .chain(
                self.sets
                    .iter()
                    .enumerate()
                    .rev()
                    .skip(1)
                    .flat_map(|(i, set)| {
                        Some(EvaluationQuery::new(
                            -((self.blinding_factors + 1) as i32),
                            x_last.clone(),
                            format!("{}_product_commitment_{}", self.key, i),
                            set.permutation_product_commitment.clone(),
                            set.permutation_product_last_eval.as_ref().unwrap().clone(),
                        ))
                    }),
            )
            .collect()
    }
}