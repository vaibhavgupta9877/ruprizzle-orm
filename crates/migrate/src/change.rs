//! The `Change` taxonomy produced by [`crate::diff::diff`].

use ruprizzle_core::ir::{EnumDef, Field, IndexDef, Model, ResolvedRelation, UniqueDef};
use ruprizzle_core::names::{EnumName, FieldName, ModelName};

/// A single schema change.
#[derive(Debug, Clone, PartialEq)]
#[allow(missing_docs)]
pub enum Change {
    CreateEnum(EnumDef),
    DropEnum(EnumName, String),
    AddEnumVariant {
        enum_: EnumName,
        variant: String,
    },
    DropEnumVariant {
        enum_: EnumName,
        variant: String,
    },

    CreateModel(Model),
    DropModel(ModelName, String),
    RenameModel {
        from: ModelName,
        to: ModelName,
        new_table: String,
    },

    AddColumn {
        model: ModelName,
        field: Field,
    },
    DropColumn {
        model: ModelName,
        column: String,
    },
    AlterColumn {
        model: ModelName,
        from: Field,
        to: Field,
        aspects: Vec<ColumnAspect>,
    },
    RenameColumn {
        model: ModelName,
        from: FieldName,
        to: FieldName,
        from_column: String,
        to_column: String,
    },

    CreateIndex(ModelName, IndexDef),
    DropIndex(ModelName, String),
    AddUnique(ModelName, UniqueDef),
    DropUnique(ModelName, String),

    AddForeignKey(ModelName, ResolvedRelation),
    DropForeignKey(ModelName, ResolvedRelation),
}

/// Which aspects of a column changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum ColumnAspect {
    Type,
    Nullability,
    Default,
    Identity,
}

impl Change {
    /// Whether this change can delete user data.
    #[must_use]
    pub fn is_destructive(&self) -> bool {
        matches!(
            self,
            Change::DropEnum(_, _)
                | Change::DropEnumVariant { .. }
                | Change::DropModel(_, _)
                | Change::DropColumn { .. }
                | Change::DropIndex(_, _)
                | Change::DropUnique(_, _)
                | Change::DropForeignKey(_, _)
        )
    }

    /// A human-readable description of the change, used in destructive-change
    /// warnings and status output.
    #[must_use]
    pub fn description(&self) -> String {
        match self {
            Change::CreateEnum(e) => format!("CREATE ENUM {}", e.name),
            Change::DropEnum(e, _) => format!("DROP ENUM {}", e),
            Change::AddEnumVariant { enum_, variant } => {
                format!("ADD ENUM VARIANT {enum_}.{variant}")
            }
            Change::DropEnumVariant { enum_, variant } => {
                format!("DROP ENUM VARIANT {enum_}.{variant}")
            }
            Change::CreateModel(m) => format!("CREATE TABLE {}", m.table),
            Change::DropModel(m, table) => format!("DROP TABLE {table} ({m})"),
            Change::RenameModel { from, to, .. } => format!("RENAME TABLE {from} -> {to}"),
            Change::AddColumn { model, field } => {
                format!("ADD COLUMN {}.{}", model, field.column)
            }
            Change::DropColumn { model, column } => format!("DROP COLUMN {model}.{column}"),
            Change::AlterColumn {
                model, from, to, ..
            } => {
                format!("ALTER COLUMN {model}.{} -> {}", from.name, to.column)
            }
            Change::RenameColumn {
                model, from, to, ..
            } => {
                format!("RENAME COLUMN {model}.{from} -> {to}")
            }
            Change::CreateIndex(m, ix) => format!("CREATE INDEX {} ON {}", ix.db_name, m),
            Change::DropIndex(m, name) => format!("DROP INDEX {name} ON {m}"),
            Change::AddUnique(m, uq) => format!("ADD UNIQUE {} ON {}", uq.db_name, m),
            Change::DropUnique(m, name) => format!("DROP UNIQUE {name} ON {m}"),
            Change::AddForeignKey(m, r) => {
                format!("ADD FOREIGN KEY {}.{:?} -> {}", m, r.owner_cols, r.target)
            }
            Change::DropForeignKey(m, r) => {
                format!("DROP FOREIGN KEY {} ON {}", r.constraint_name, m)
            }
        }
    }
}
