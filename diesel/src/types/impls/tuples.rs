use std::error::Error;

use associations::BelongsTo;
use backend::{Backend, SupportsDefaultKeyword};
use expression::{Expression, SelectableExpression, AppearsOnTable, NonAggregate};
use insertable::{ColumnInsertValue, InsertValues};
use query_builder::*;
use query_source::{QuerySource, Queryable, Table, Column};
use result::QueryResult;
use row::Row;
use types::{HasSqlType, FromSqlRow, Nullable, NotNull};
use util::TupleAppend;

macro_rules! tuple_impls {
    ($(
        $Tuple:tt {
            $(($idx:tt) -> $T:ident, $ST:ident, $TT:ident,)+
    }
    )+) => {
        $(
            impl<$($T),+, DB> HasSqlType<($($T,)+)> for DB where
                $(DB: HasSqlType<$T>),+,
                DB: Backend,
            {
                fn metadata(_: &DB::MetadataLookup) -> DB::TypeMetadata {
                    unreachable!("Tuples should never implement `ToSql` directly");
                }

                fn row_metadata(out: &mut Vec<DB::TypeMetadata>, lookup: &DB::MetadataLookup) {
                    $(<DB as HasSqlType<$T>>::row_metadata(out, lookup);)+
                }
            }

            impl<$($T),+> NotNull for ($($T,)+) {
            }

            impl<$($T),+, $($ST),+, DB> FromSqlRow<($($ST,)+), DB> for ($($T,)+) where
                DB: Backend,
                $($T: FromSqlRow<$ST, DB>),+,
                $(DB: HasSqlType<$ST>),+,
                DB: HasSqlType<($($ST,)+)>,
            {
                fn build_from_row<RowT: Row<DB>>(row: &mut RowT) -> Result<Self, Box<Error+Send+Sync>> {
                    Ok(($(try!($T::build_from_row(row)),)+))
                }

                fn fields_needed() -> usize {
                    $($T::fields_needed() +)+ 0
                }
            }

            impl<$($T),+, $($ST),+, DB> FromSqlRow<Nullable<($($ST,)+)>, DB> for Option<($($T,)+)> where
                DB: Backend,
                $($T: FromSqlRow<$ST, DB>),+,
                $(DB: HasSqlType<$ST>),+,
                DB: HasSqlType<($($ST,)+)>,
            {
                fn build_from_row<RowT: Row<DB>>(row: &mut RowT) -> Result<Self, Box<Error+Send+Sync>> {
                    let fields_needed = <Self as FromSqlRow<Nullable<($($ST,)+)>, DB>>::fields_needed();
                    if row.next_is_null(fields_needed) {
                        row.advance(fields_needed);
                        Ok(None)
                    } else {
                        Ok(Some(($(try!($T::build_from_row(row)),)+)))
                    }
                }

                fn fields_needed() -> usize {
                    $($T::fields_needed() +)+ 0
                }
            }

            impl<$($T),+, $($ST),+, DB> Queryable<($($ST,)+), DB> for ($($T,)+) where
                DB: Backend,
                $($T: Queryable<$ST, DB>),+,
                $(DB: HasSqlType<$ST>),+,
                DB: HasSqlType<($($ST,)+)>,
            {
                type Row = ($($T::Row,)+);

                fn build(row: Self::Row) -> Self {
                    ($($T::build(row.$idx),)+)
                }
            }

            impl<$($T: Expression + NonAggregate),+> Expression for ($($T,)+) {
                type SqlType = ($(<$T as Expression>::SqlType,)+);
            }

            impl<$($T: QueryFragment<DB>),+, DB: Backend> QueryFragment<DB> for ($($T,)+) {
                fn walk_ast(&self, mut out: AstPass<DB>) -> QueryResult<()> {
                    $(
                        if $idx != 0 {
                            out.push_sql(", ");
                        }
                        self.$idx.walk_ast(out.reborrow())?;
                    )+
                    Ok(())
                }
            }

            impl<$($T: QueryId),+> QueryId for ($($T,)+) {
                type QueryId = ($($T::QueryId,)+);

                fn has_static_query_id() -> bool {
                    $($T::has_static_query_id() &&)+ true
                }
            }

            impl<$($T: Expression + NonAggregate),+> NonAggregate for ($($T,)+) {
            }

            #[cfg_attr(feature = "clippy", allow(eq_op))] // Clippy doesn't like the trivial case for 1-tuples
            impl<$($T,)+ $($ST,)+ Tab, DB> InsertValues<DB>
                for ($(ColumnInsertValue<$T, $ST>,)+) where
                    DB: Backend + SupportsDefaultKeyword,
                    Tab: Table,
                    $($T: Column<Table=Tab>,)+
                    $($ST: Expression<SqlType=$T::SqlType> + QueryFragment<DB>,)+
            {
                fn column_names(&self, out: &mut DB::QueryBuilder) -> QueryResult<()> {
                    $(
                        if $idx != 0 {
                            out.push_sql(", ");
                        }
                        try!(out.push_identifier($T::name()));
                    )+
                    Ok(())
                }

                fn walk_ast(&self, mut out: AstPass<DB>) -> QueryResult<()> {
                    out.push_sql("(");
                    $(
                        if $idx != 0 {
                            out.push_sql(", ");
                        }
                        if let ColumnInsertValue::Expression(_, ref value) = self.$idx {
                            value.walk_ast(out.reborrow())?;
                        } else {
                            out.push_sql("DEFAULT");
                        }
                    )+
                    out.push_sql(")");
                    Ok(())
                }
            }

            #[cfg(feature = "sqlite")]
            impl<$($T,)+ $($ST,)+ Tab> InsertValues<::sqlite::Sqlite>
                for ($(ColumnInsertValue<$T, $ST>,)+) where
                    Tab: Table,
                    $($T: Column<Table=Tab>,)+
                    $($ST: Expression<SqlType=$T::SqlType> + QueryFragment<::sqlite::Sqlite>,)+
            {
                #[allow(unused_assignments)]
                fn column_names(&self, out: &mut ::sqlite::SqliteQueryBuilder) -> QueryResult<()> {
                    let mut columns_present = false;
                    $(
                        if let ColumnInsertValue::Expression(..) = self.$idx {
                            if columns_present {
                                out.push_sql(", ");
                            }
                            try!(out.push_identifier($T::name()));
                            columns_present = true;
                        }
                    )+
                    Ok(())
                }

                #[allow(unused_assignments)]
                fn walk_ast(&self, mut out: AstPass<::sqlite::Sqlite>) -> QueryResult<()> {
                    out.push_sql("(");
                    let mut columns_present = false;
                    $(
                        if let ColumnInsertValue::Expression(_, ref value) = self.$idx {
                            if columns_present {
                                out.push_sql(", ");
                            }
                            value.walk_ast(out.reborrow())?;
                            columns_present = true;
                        }
                    )+
                    out.push_sql(")");
                    Ok(())
                }
            }

            impl<$($T,)+ QS> SelectableExpression<QS> for ($($T,)+) where
                $($T: SelectableExpression<QS>,)+
                ($($T,)+): AppearsOnTable<QS>,
            {
            }

            impl<$($T,)+ QS> AppearsOnTable<QS> for ($($T,)+) where
                $($T: AppearsOnTable<QS>,)+
                ($($T,)+): Expression,
            {
            }

            impl<Target, $($T,)+> AsChangeset for ($($T,)+) where
                $($T: AsChangeset<Target=Target>,)+
                Target: QuerySource,
            {
                type Target = Target;
                type Changeset = ($($T::Changeset,)+);

                fn as_changeset(self) -> Self::Changeset {
                    ($(self.$idx.as_changeset(),)+)
                }
            }

            impl<DB, $($T,)+> Changeset<DB> for ($($T,)+) where
                DB: Backend,
                $($T: Changeset<DB>,)+
            {
                fn is_noop(&self) -> bool {
                    $(self.$idx.is_noop() &&)+ true
                }

                #[allow(unused_assignments)]
                fn walk_ast(&self, mut out: AstPass<DB>) -> QueryResult<()> {
                    let mut needs_comma = false;
                    $(
                        let noop_element = self.$idx.is_noop();
                        if !noop_element {
                            if needs_comma {
                                out.push_sql(", ");
                            }
                            self.$idx.walk_ast(out.reborrow())?;
                            needs_comma = true;
                        }
                    )+
                    Ok(())
                }
            }

            impl<$($T,)+ Parent> BelongsTo<Parent> for ($($T,)+) where
                A: BelongsTo<Parent>,
            {
                type ForeignKey = A::ForeignKey;
                type ForeignKeyColumn = A::ForeignKeyColumn;

                fn foreign_key(&self) -> Option<&Self::ForeignKey> {
                    self.0.foreign_key()
                }

                fn foreign_key_column() -> Self::ForeignKeyColumn {
                    A::foreign_key_column()
                }
            }

            impl<$($T,)+ Next> TupleAppend<Next> for ($($T,)+) {
                type Output = ($($T,)+ Next);

                #[allow(non_snake_case)]
                fn tuple_append(self, next: Next) -> Self::Output {
                    let ($($T,)+) = self;
                    ($($T,)+ next)
                }
            }
        )+
    }
}

tuple_impls! {
    1 {
        (0) -> A, SA, TA,
    }
    2 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
    }
    3 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
    }
    4 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
    }
    5 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
    }
    6 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
    }
    7 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
    }
    8 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
    }
    9 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
    }
    10 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
    }
    11 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
    }
    12 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
    }
    13 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
    }
    14 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
    }
    15 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
    }
    16 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
    }
}

#[cfg(feature = "large-tables")]
tuple_impls! {
    17 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
    }
    18 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
        (17) -> R, SR, TR,
    }
    19 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
        (17) -> R, SR, TR,
        (18) -> S, SS, TS,
    }
    20 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
        (17) -> R, SR, TR,
        (18) -> S, SS, TS,
        (19) -> T, ST, TT,
    }
    21 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
        (17) -> R, SR, TR,
        (18) -> S, SS, TS,
        (19) -> T, ST, TT,
        (20) -> U, SU, TU,
    }
    22 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
        (17) -> R, SR, TR,
        (18) -> S, SS, TS,
        (19) -> T, ST, TT,
        (20) -> U, SU, TU,
        (21) -> V, SV, TV,
    }
    23 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
        (17) -> R, SR, TR,
        (18) -> S, SS, TS,
        (19) -> T, ST, TT,
        (20) -> U, SU, TU,
        (21) -> V, SV, TV,
        (22) -> W, SW, TW,
    }
    24 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
        (17) -> R, SR, TR,
        (18) -> S, SS, TS,
        (19) -> T, ST, TT,
        (20) -> U, SU, TU,
        (21) -> V, SV, TV,
        (22) -> W, SW, TW,
        (23) -> X, SX, TX,
    }
    25 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
        (17) -> R, SR, TR,
        (18) -> S, SS, TS,
        (19) -> T, ST, TT,
        (20) -> U, SU, TU,
        (21) -> V, SV, TV,
        (22) -> W, SW, TW,
        (23) -> X, SX, TX,
        (24) -> Y, SY, TY,
    }
    26 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
        (17) -> R, SR, TR,
        (18) -> S, SS, TS,
        (19) -> T, ST, TT,
        (20) -> U, SU, TU,
        (21) -> V, SV, TV,
        (22) -> W, SW, TW,
        (23) -> X, SX, TX,
        (24) -> Y, SY, TY,
        (25) -> Z, SZ, TZ,
    }
}

#[cfg(feature = "huge-tables")]
tuple_impls! {
    27 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
        (17) -> R, SR, TR,
        (18) -> S, SS, TS,
        (19) -> T, ST, TT,
        (20) -> U, SU, TU,
        (21) -> V, SV, TV,
        (22) -> W, SW, TW,
        (23) -> X, SX, TX,
        (24) -> Y, SY, TY,
        (25) -> Z, SZ, TZ,
        (26) -> AA, SAA, TAA,
    }
    28 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
        (17) -> R, SR, TR,
        (18) -> S, SS, TS,
        (19) -> T, ST, TT,
        (20) -> U, SU, TU,
        (21) -> V, SV, TV,
        (22) -> W, SW, TW,
        (23) -> X, SX, TX,
        (24) -> Y, SY, TY,
        (25) -> Z, SZ, TZ,
        (26) -> AA, SAA, TAA,
        (27) -> AB, SAB, TAB,
    }
    29 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
        (17) -> R, SR, TR,
        (18) -> S, SS, TS,
        (19) -> T, ST, TT,
        (20) -> U, SU, TU,
        (21) -> V, SV, TV,
        (22) -> W, SW, TW,
        (23) -> X, SX, TX,
        (24) -> Y, SY, TY,
        (25) -> Z, SZ, TZ,
        (26) -> AA, SAA, TAA,
        (27) -> AB, SAB, TAB,
        (28) -> AC, SAC, TAC,
    }
    30 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
        (17) -> R, SR, TR,
        (18) -> S, SS, TS,
        (19) -> T, ST, TT,
        (20) -> U, SU, TU,
        (21) -> V, SV, TV,
        (22) -> W, SW, TW,
        (23) -> X, SX, TX,
        (24) -> Y, SY, TY,
        (25) -> Z, SZ, TZ,
        (26) -> AA, SAA, TAA,
        (27) -> AB, SAB, TAB,
        (28) -> AC, SAC, TAC,
        (29) -> AD, SAD, TAD,
    }
    31 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
        (17) -> R, SR, TR,
        (18) -> S, SS, TS,
        (19) -> T, ST, TT,
        (20) -> U, SU, TU,
        (21) -> V, SV, TV,
        (22) -> W, SW, TW,
        (23) -> X, SX, TX,
        (24) -> Y, SY, TY,
        (25) -> Z, SZ, TZ,
        (26) -> AA, SAA, TAA,
        (27) -> AB, SAB, TAB,
        (28) -> AC, SAC, TAC,
        (29) -> AD, SAD, TAD,
        (30) -> AE, SAE, TAE,
    }
    32 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
        (17) -> R, SR, TR,
        (18) -> S, SS, TS,
        (19) -> T, ST, TT,
        (20) -> U, SU, TU,
        (21) -> V, SV, TV,
        (22) -> W, SW, TW,
        (23) -> X, SX, TX,
        (24) -> Y, SY, TY,
        (25) -> Z, SZ, TZ,
        (26) -> AA, SAA, TAA,
        (27) -> AB, SAB, TAB,
        (28) -> AC, SAC, TAC,
        (29) -> AD, SAD, TAD,
        (30) -> AE, SAE, TAE,
        (31) -> AF, SAF, TAF,
    }
    33 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
        (17) -> R, SR, TR,
        (18) -> S, SS, TS,
        (19) -> T, ST, TT,
        (20) -> U, SU, TU,
        (21) -> V, SV, TV,
        (22) -> W, SW, TW,
        (23) -> X, SX, TX,
        (24) -> Y, SY, TY,
        (25) -> Z, SZ, TZ,
        (26) -> AA, SAA, TAA,
        (27) -> AB, SAB, TAB,
        (28) -> AC, SAC, TAC,
        (29) -> AD, SAD, TAD,
        (30) -> AE, SAE, TAE,
        (31) -> AF, SAF, TAF,
        (32) -> AG, SAG, TAG,
    }
    34 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
        (17) -> R, SR, TR,
        (18) -> S, SS, TS,
        (19) -> T, ST, TT,
        (20) -> U, SU, TU,
        (21) -> V, SV, TV,
        (22) -> W, SW, TW,
        (23) -> X, SX, TX,
        (24) -> Y, SY, TY,
        (25) -> Z, SZ, TZ,
        (26) -> AA, SAA, TAA,
        (27) -> AB, SAB, TAB,
        (28) -> AC, SAC, TAC,
        (29) -> AD, SAD, TAD,
        (30) -> AE, SAE, TAE,
        (31) -> AF, SAF, TAF,
        (32) -> AG, SAG, TAG,
        (33) -> AH, SAH, TAH,
    }
    35 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
        (17) -> R, SR, TR,
        (18) -> S, SS, TS,
        (19) -> T, ST, TT,
        (20) -> U, SU, TU,
        (21) -> V, SV, TV,
        (22) -> W, SW, TW,
        (23) -> X, SX, TX,
        (24) -> Y, SY, TY,
        (25) -> Z, SZ, TZ,
        (26) -> AA, SAA, TAA,
        (27) -> AB, SAB, TAB,
        (28) -> AC, SAC, TAC,
        (29) -> AD, SAD, TAD,
        (30) -> AE, SAE, TAE,
        (31) -> AF, SAF, TAF,
        (32) -> AG, SAG, TAG,
        (33) -> AH, SAH, TAH,
        (34) -> AI, SAI, TAI,
    }
    36 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
        (17) -> R, SR, TR,
        (18) -> S, SS, TS,
        (19) -> T, ST, TT,
        (20) -> U, SU, TU,
        (21) -> V, SV, TV,
        (22) -> W, SW, TW,
        (23) -> X, SX, TX,
        (24) -> Y, SY, TY,
        (25) -> Z, SZ, TZ,
        (26) -> AA, SAA, TAA,
        (27) -> AB, SAB, TAB,
        (28) -> AC, SAC, TAC,
        (29) -> AD, SAD, TAD,
        (30) -> AE, SAE, TAE,
        (31) -> AF, SAF, TAF,
        (32) -> AG, SAG, TAG,
        (33) -> AH, SAH, TAH,
        (34) -> AI, SAI, TAI,
        (35) -> AJ, SAJ, TAJ,
    }
    37 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
        (17) -> R, SR, TR,
        (18) -> S, SS, TS,
        (19) -> T, ST, TT,
        (20) -> U, SU, TU,
        (21) -> V, SV, TV,
        (22) -> W, SW, TW,
        (23) -> X, SX, TX,
        (24) -> Y, SY, TY,
        (25) -> Z, SZ, TZ,
        (26) -> AA, SAA, TAA,
        (27) -> AB, SAB, TAB,
        (28) -> AC, SAC, TAC,
        (29) -> AD, SAD, TAD,
        (30) -> AE, SAE, TAE,
        (31) -> AF, SAF, TAF,
        (32) -> AG, SAG, TAG,
        (33) -> AH, SAH, TAH,
        (34) -> AI, SAI, TAI,
        (35) -> AJ, SAJ, TAJ,
        (36) -> AK, SAK, TAK,
    }
    38 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
        (17) -> R, SR, TR,
        (18) -> S, SS, TS,
        (19) -> T, ST, TT,
        (20) -> U, SU, TU,
        (21) -> V, SV, TV,
        (22) -> W, SW, TW,
        (23) -> X, SX, TX,
        (24) -> Y, SY, TY,
        (25) -> Z, SZ, TZ,
        (26) -> AA, SAA, TAA,
        (27) -> AB, SAB, TAB,
        (28) -> AC, SAC, TAC,
        (29) -> AD, SAD, TAD,
        (30) -> AE, SAE, TAE,
        (31) -> AF, SAF, TAF,
        (32) -> AG, SAG, TAG,
        (33) -> AH, SAH, TAH,
        (34) -> AI, SAI, TAI,
        (35) -> AJ, SAJ, TAJ,
        (36) -> AK, SAK, TAK,
        (37) -> AL, SAL, TAL,
    }
    39 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
        (17) -> R, SR, TR,
        (18) -> S, SS, TS,
        (19) -> T, ST, TT,
        (20) -> U, SU, TU,
        (21) -> V, SV, TV,
        (22) -> W, SW, TW,
        (23) -> X, SX, TX,
        (24) -> Y, SY, TY,
        (25) -> Z, SZ, TZ,
        (26) -> AA, SAA, TAA,
        (27) -> AB, SAB, TAB,
        (28) -> AC, SAC, TAC,
        (29) -> AD, SAD, TAD,
        (30) -> AE, SAE, TAE,
        (31) -> AF, SAF, TAF,
        (32) -> AG, SAG, TAG,
        (33) -> AH, SAH, TAH,
        (34) -> AI, SAI, TAI,
        (35) -> AJ, SAJ, TAJ,
        (36) -> AK, SAK, TAK,
        (37) -> AL, SAL, TAL,
        (38) -> AM, SAM, TAM,
    }
    40 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
        (17) -> R, SR, TR,
        (18) -> S, SS, TS,
        (19) -> T, ST, TT,
        (20) -> U, SU, TU,
        (21) -> V, SV, TV,
        (22) -> W, SW, TW,
        (23) -> X, SX, TX,
        (24) -> Y, SY, TY,
        (25) -> Z, SZ, TZ,
        (26) -> AA, SAA, TAA,
        (27) -> AB, SAB, TAB,
        (28) -> AC, SAC, TAC,
        (29) -> AD, SAD, TAD,
        (30) -> AE, SAE, TAE,
        (31) -> AF, SAF, TAF,
        (32) -> AG, SAG, TAG,
        (33) -> AH, SAH, TAH,
        (34) -> AI, SAI, TAI,
        (35) -> AJ, SAJ, TAJ,
        (36) -> AK, SAK, TAK,
        (37) -> AL, SAL, TAL,
        (38) -> AM, SAM, TAM,
        (39) -> AN, SAN, TAN,
    }
    41 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
        (17) -> R, SR, TR,
        (18) -> S, SS, TS,
        (19) -> T, ST, TT,
        (20) -> U, SU, TU,
        (21) -> V, SV, TV,
        (22) -> W, SW, TW,
        (23) -> X, SX, TX,
        (24) -> Y, SY, TY,
        (25) -> Z, SZ, TZ,
        (26) -> AA, SAA, TAA,
        (27) -> AB, SAB, TAB,
        (28) -> AC, SAC, TAC,
        (29) -> AD, SAD, TAD,
        (30) -> AE, SAE, TAE,
        (31) -> AF, SAF, TAF,
        (32) -> AG, SAG, TAG,
        (33) -> AH, SAH, TAH,
        (34) -> AI, SAI, TAI,
        (35) -> AJ, SAJ, TAJ,
        (36) -> AK, SAK, TAK,
        (37) -> AL, SAL, TAL,
        (38) -> AM, SAM, TAM,
        (39) -> AN, SAN, TAN,
        (40) -> AO, SAO, TAO,
    }
    42 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
        (17) -> R, SR, TR,
        (18) -> S, SS, TS,
        (19) -> T, ST, TT,
        (20) -> U, SU, TU,
        (21) -> V, SV, TV,
        (22) -> W, SW, TW,
        (23) -> X, SX, TX,
        (24) -> Y, SY, TY,
        (25) -> Z, SZ, TZ,
        (26) -> AA, SAA, TAA,
        (27) -> AB, SAB, TAB,
        (28) -> AC, SAC, TAC,
        (29) -> AD, SAD, TAD,
        (30) -> AE, SAE, TAE,
        (31) -> AF, SAF, TAF,
        (32) -> AG, SAG, TAG,
        (33) -> AH, SAH, TAH,
        (34) -> AI, SAI, TAI,
        (35) -> AJ, SAJ, TAJ,
        (36) -> AK, SAK, TAK,
        (37) -> AL, SAL, TAL,
        (38) -> AM, SAM, TAM,
        (39) -> AN, SAN, TAN,
        (40) -> AO, SAO, TAO,
        (41) -> AP, SAP, TAP,
    }
    43 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
        (17) -> R, SR, TR,
        (18) -> S, SS, TS,
        (19) -> T, ST, TT,
        (20) -> U, SU, TU,
        (21) -> V, SV, TV,
        (22) -> W, SW, TW,
        (23) -> X, SX, TX,
        (24) -> Y, SY, TY,
        (25) -> Z, SZ, TZ,
        (26) -> AA, SAA, TAA,
        (27) -> AB, SAB, TAB,
        (28) -> AC, SAC, TAC,
        (29) -> AD, SAD, TAD,
        (30) -> AE, SAE, TAE,
        (31) -> AF, SAF, TAF,
        (32) -> AG, SAG, TAG,
        (33) -> AH, SAH, TAH,
        (34) -> AI, SAI, TAI,
        (35) -> AJ, SAJ, TAJ,
        (36) -> AK, SAK, TAK,
        (37) -> AL, SAL, TAL,
        (38) -> AM, SAM, TAM,
        (39) -> AN, SAN, TAN,
        (40) -> AO, SAO, TAO,
        (41) -> AP, SAP, TAP,
        (42) -> AQ, SAQ, TAQ,
    }
    44 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
        (17) -> R, SR, TR,
        (18) -> S, SS, TS,
        (19) -> T, ST, TT,
        (20) -> U, SU, TU,
        (21) -> V, SV, TV,
        (22) -> W, SW, TW,
        (23) -> X, SX, TX,
        (24) -> Y, SY, TY,
        (25) -> Z, SZ, TZ,
        (26) -> AA, SAA, TAA,
        (27) -> AB, SAB, TAB,
        (28) -> AC, SAC, TAC,
        (29) -> AD, SAD, TAD,
        (30) -> AE, SAE, TAE,
        (31) -> AF, SAF, TAF,
        (32) -> AG, SAG, TAG,
        (33) -> AH, SAH, TAH,
        (34) -> AI, SAI, TAI,
        (35) -> AJ, SAJ, TAJ,
        (36) -> AK, SAK, TAK,
        (37) -> AL, SAL, TAL,
        (38) -> AM, SAM, TAM,
        (39) -> AN, SAN, TAN,
        (40) -> AO, SAO, TAO,
        (41) -> AP, SAP, TAP,
        (42) -> AQ, SAQ, TAQ,
        (43) -> AR, SAR, TAR,
    }
    45 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
        (17) -> R, SR, TR,
        (18) -> S, SS, TS,
        (19) -> T, ST, TT,
        (20) -> U, SU, TU,
        (21) -> V, SV, TV,
        (22) -> W, SW, TW,
        (23) -> X, SX, TX,
        (24) -> Y, SY, TY,
        (25) -> Z, SZ, TZ,
        (26) -> AA, SAA, TAA,
        (27) -> AB, SAB, TAB,
        (28) -> AC, SAC, TAC,
        (29) -> AD, SAD, TAD,
        (30) -> AE, SAE, TAE,
        (31) -> AF, SAF, TAF,
        (32) -> AG, SAG, TAG,
        (33) -> AH, SAH, TAH,
        (34) -> AI, SAI, TAI,
        (35) -> AJ, SAJ, TAJ,
        (36) -> AK, SAK, TAK,
        (37) -> AL, SAL, TAL,
        (38) -> AM, SAM, TAM,
        (39) -> AN, SAN, TAN,
        (40) -> AO, SAO, TAO,
        (41) -> AP, SAP, TAP,
        (42) -> AQ, SAQ, TAQ,
        (43) -> AR, SAR, TAR,
        (44) -> AS, SAS, TAS,
    }
    46 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
        (17) -> R, SR, TR,
        (18) -> S, SS, TS,
        (19) -> T, ST, TT,
        (20) -> U, SU, TU,
        (21) -> V, SV, TV,
        (22) -> W, SW, TW,
        (23) -> X, SX, TX,
        (24) -> Y, SY, TY,
        (25) -> Z, SZ, TZ,
        (26) -> AA, SAA, TAA,
        (27) -> AB, SAB, TAB,
        (28) -> AC, SAC, TAC,
        (29) -> AD, SAD, TAD,
        (30) -> AE, SAE, TAE,
        (31) -> AF, SAF, TAF,
        (32) -> AG, SAG, TAG,
        (33) -> AH, SAH, TAH,
        (34) -> AI, SAI, TAI,
        (35) -> AJ, SAJ, TAJ,
        (36) -> AK, SAK, TAK,
        (37) -> AL, SAL, TAL,
        (38) -> AM, SAM, TAM,
        (39) -> AN, SAN, TAN,
        (40) -> AO, SAO, TAO,
        (41) -> AP, SAP, TAP,
        (42) -> AQ, SAQ, TAQ,
        (43) -> AR, SAR, TAR,
        (44) -> AS, SAS, TAS,
        (45) -> AT, SAT, TAT,
    }
    47 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
        (17) -> R, SR, TR,
        (18) -> S, SS, TS,
        (19) -> T, ST, TT,
        (20) -> U, SU, TU,
        (21) -> V, SV, TV,
        (22) -> W, SW, TW,
        (23) -> X, SX, TX,
        (24) -> Y, SY, TY,
        (25) -> Z, SZ, TZ,
        (26) -> AA, SAA, TAA,
        (27) -> AB, SAB, TAB,
        (28) -> AC, SAC, TAC,
        (29) -> AD, SAD, TAD,
        (30) -> AE, SAE, TAE,
        (31) -> AF, SAF, TAF,
        (32) -> AG, SAG, TAG,
        (33) -> AH, SAH, TAH,
        (34) -> AI, SAI, TAI,
        (35) -> AJ, SAJ, TAJ,
        (36) -> AK, SAK, TAK,
        (37) -> AL, SAL, TAL,
        (38) -> AM, SAM, TAM,
        (39) -> AN, SAN, TAN,
        (40) -> AO, SAO, TAO,
        (41) -> AP, SAP, TAP,
        (42) -> AQ, SAQ, TAQ,
        (43) -> AR, SAR, TAR,
        (44) -> AS, SAS, TAS,
        (45) -> AT, SAT, TAT,
        (46) -> AU, SAU, TAU,
    }
    48 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
        (17) -> R, SR, TR,
        (18) -> S, SS, TS,
        (19) -> T, ST, TT,
        (20) -> U, SU, TU,
        (21) -> V, SV, TV,
        (22) -> W, SW, TW,
        (23) -> X, SX, TX,
        (24) -> Y, SY, TY,
        (25) -> Z, SZ, TZ,
        (26) -> AA, SAA, TAA,
        (27) -> AB, SAB, TAB,
        (28) -> AC, SAC, TAC,
        (29) -> AD, SAD, TAD,
        (30) -> AE, SAE, TAE,
        (31) -> AF, SAF, TAF,
        (32) -> AG, SAG, TAG,
        (33) -> AH, SAH, TAH,
        (34) -> AI, SAI, TAI,
        (35) -> AJ, SAJ, TAJ,
        (36) -> AK, SAK, TAK,
        (37) -> AL, SAL, TAL,
        (38) -> AM, SAM, TAM,
        (39) -> AN, SAN, TAN,
        (40) -> AO, SAO, TAO,
        (41) -> AP, SAP, TAP,
        (42) -> AQ, SAQ, TAQ,
        (43) -> AR, SAR, TAR,
        (44) -> AS, SAS, TAS,
        (45) -> AT, SAT, TAT,
        (46) -> AU, SAU, TAU,
        (47) -> AV, SAV, TAV,
    }
    49 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
        (17) -> R, SR, TR,
        (18) -> S, SS, TS,
        (19) -> T, ST, TT,
        (20) -> U, SU, TU,
        (21) -> V, SV, TV,
        (22) -> W, SW, TW,
        (23) -> X, SX, TX,
        (24) -> Y, SY, TY,
        (25) -> Z, SZ, TZ,
        (26) -> AA, SAA, TAA,
        (27) -> AB, SAB, TAB,
        (28) -> AC, SAC, TAC,
        (29) -> AD, SAD, TAD,
        (30) -> AE, SAE, TAE,
        (31) -> AF, SAF, TAF,
        (32) -> AG, SAG, TAG,
        (33) -> AH, SAH, TAH,
        (34) -> AI, SAI, TAI,
        (35) -> AJ, SAJ, TAJ,
        (36) -> AK, SAK, TAK,
        (37) -> AL, SAL, TAL,
        (38) -> AM, SAM, TAM,
        (39) -> AN, SAN, TAN,
        (40) -> AO, SAO, TAO,
        (41) -> AP, SAP, TAP,
        (42) -> AQ, SAQ, TAQ,
        (43) -> AR, SAR, TAR,
        (44) -> AS, SAS, TAS,
        (45) -> AT, SAT, TAT,
        (46) -> AU, SAU, TAU,
        (47) -> AV, SAV, TAV,
        (48) -> AW, SAW, TAW,
    }
    50 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
        (17) -> R, SR, TR,
        (18) -> S, SS, TS,
        (19) -> T, ST, TT,
        (20) -> U, SU, TU,
        (21) -> V, SV, TV,
        (22) -> W, SW, TW,
        (23) -> X, SX, TX,
        (24) -> Y, SY, TY,
        (25) -> Z, SZ, TZ,
        (26) -> AA, SAA, TAA,
        (27) -> AB, SAB, TAB,
        (28) -> AC, SAC, TAC,
        (29) -> AD, SAD, TAD,
        (30) -> AE, SAE, TAE,
        (31) -> AF, SAF, TAF,
        (32) -> AG, SAG, TAG,
        (33) -> AH, SAH, TAH,
        (34) -> AI, SAI, TAI,
        (35) -> AJ, SAJ, TAJ,
        (36) -> AK, SAK, TAK,
        (37) -> AL, SAL, TAL,
        (38) -> AM, SAM, TAM,
        (39) -> AN, SAN, TAN,
        (40) -> AO, SAO, TAO,
        (41) -> AP, SAP, TAP,
        (42) -> AQ, SAQ, TAQ,
        (43) -> AR, SAR, TAR,
        (44) -> AS, SAS, TAS,
        (45) -> AT, SAT, TAT,
        (46) -> AU, SAU, TAU,
        (47) -> AV, SAV, TAV,
        (48) -> AW, SAW, TAW,
        (49) -> AX, SAX, TAX,
    }
    51 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
        (17) -> R, SR, TR,
        (18) -> S, SS, TS,
        (19) -> T, ST, TT,
        (20) -> U, SU, TU,
        (21) -> V, SV, TV,
        (22) -> W, SW, TW,
        (23) -> X, SX, TX,
        (24) -> Y, SY, TY,
        (25) -> Z, SZ, TZ,
        (26) -> AA, SAA, TAA,
        (27) -> AB, SAB, TAB,
        (28) -> AC, SAC, TAC,
        (29) -> AD, SAD, TAD,
        (30) -> AE, SAE, TAE,
        (31) -> AF, SAF, TAF,
        (32) -> AG, SAG, TAG,
        (33) -> AH, SAH, TAH,
        (34) -> AI, SAI, TAI,
        (35) -> AJ, SAJ, TAJ,
        (36) -> AK, SAK, TAK,
        (37) -> AL, SAL, TAL,
        (38) -> AM, SAM, TAM,
        (39) -> AN, SAN, TAN,
        (40) -> AO, SAO, TAO,
        (41) -> AP, SAP, TAP,
        (42) -> AQ, SAQ, TAQ,
        (43) -> AR, SAR, TAR,
        (44) -> AS, SAS, TAS,
        (45) -> AT, SAT, TAT,
        (46) -> AU, SAU, TAU,
        (47) -> AV, SAV, TAV,
        (48) -> AW, SAW, TAW,
        (49) -> AX, SAX, TAX,
        (50) -> AY, SAY, TAY,
    }
    52 {
        (0) -> A, SA, TA,
        (1) -> B, SB, TB,
        (2) -> C, SC, TC,
        (3) -> D, SD, TD,
        (4) -> E, SE, TE,
        (5) -> F, SF, TF,
        (6) -> G, SG, TG,
        (7) -> H, SH, TH,
        (8) -> I, SI, TI,
        (9) -> J, SJ, TJ,
        (10) -> K, SK, TK,
        (11) -> L, SL, TL,
        (12) -> M, SM, TM,
        (13) -> N, SN, TN,
        (14) -> O, SO, TO,
        (15) -> P, SP, TP,
        (16) -> Q, SQ, TQ,
        (17) -> R, SR, TR,
        (18) -> S, SS, TS,
        (19) -> T, ST, TT,
        (20) -> U, SU, TU,
        (21) -> V, SV, TV,
        (22) -> W, SW, TW,
        (23) -> X, SX, TX,
        (24) -> Y, SY, TY,
        (25) -> Z, SZ, TZ,
        (26) -> AA, SAA, TAA,
        (27) -> AB, SAB, TAB,
        (28) -> AC, SAC, TAC,
        (29) -> AD, SAD, TAD,
        (30) -> AE, SAE, TAE,
        (31) -> AF, SAF, TAF,
        (32) -> AG, SAG, TAG,
        (33) -> AH, SAH, TAH,
        (34) -> AI, SAI, TAI,
        (35) -> AJ, SAJ, TAJ,
        (36) -> AK, SAK, TAK,
        (37) -> AL, SAL, TAL,
        (38) -> AM, SAM, TAM,
        (39) -> AN, SAN, TAN,
        (40) -> AO, SAO, TAO,
        (41) -> AP, SAP, TAP,
        (42) -> AQ, SAQ, TAQ,
        (43) -> AR, SAR, TAR,
        (44) -> AS, SAS, TAS,
        (45) -> AT, SAT, TAT,
        (46) -> AU, SAU, TAU,
        (47) -> AV, SAV, TAV,
        (48) -> AW, SAW, TAW,
        (49) -> AX, SAX, TAX,
        (50) -> AY, SAY, TAY,
        (51) -> AZ, SAZ, TAZ,
    }
}
