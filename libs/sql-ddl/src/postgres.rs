use std::fmt::{Display, Write as _};

enum PostgresIdentifier<T> {
    Simple(T),
    WithSchema(T, T),
}

impl<T> Display for PostgresIdentifier<T>
where
    T: Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PostgresIdentifier::Simple(ident) => write!(f, "\"{}\"", ident),
            PostgresIdentifier::WithSchema(schema_name, ident) => write!(f, "\"{}\".\"{}\"", schema_name, ident),
        }
    }
}

pub struct CreateEnum<T> {
    enum_name: PostgresIdentifier<T>,
    variants: String,
}

impl<T: Display> CreateEnum<T> {
    pub fn named(enum_name: T) -> CreateEnum<T> {
        CreateEnum {
            enum_name: PostgresIdentifier::Simple(enum_name),
            variants: String::new(),
        }
    }

    pub fn named_with_schema(schema_name: T, enum_name: T) -> CreateEnum<T> {
        CreateEnum {
            enum_name: PostgresIdentifier::WithSchema(schema_name, enum_name),
            variants: String::new(),
        }
    }

    pub fn with_variants<U, V>(mut self, variants: V) -> Self
    where
        V: Iterator<Item = U>,
        U: Display,
    {
        self.variants.clear();
        self.variants.reserve(variants.size_hint().0 * 3);

        let mut variants = variants.peekable();

        while let Some(variant) = variants.next() {
            write!(self.variants, "'{}'", variant).expect("Failure writing to string.");

            if variants.peek().is_some() {
                self.variants.push_str(", ");
            }
        }

        self
    }
}

impl<T> Display for CreateEnum<T>
where
    T: Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "CREATE TYPE {enum_name} AS ENUM ({variants})",
            enum_name = self.enum_name,
            variants = self.variants,
        )
    }
}

pub struct CreateIndex<T> {
    index_name: PostgresIdentifier<T>,
    is_unique: bool,
    table_reference: PostgresIdentifier<T>,
    columns: String,
}

impl<T: Display> CreateIndex<T> {
    pub fn new(name: T, is_unique: bool, table_reference: T) -> CreateIndex<T> {
        CreateIndex {
            index_name: PostgresIdentifier::Simple(name),
            is_unique,
            table_reference: PostgresIdentifier::Simple(table_reference),
            columns: String::new(),
        }
    }

    pub fn with_columns<U, V>(mut self, columns: V) -> Self
    where
        V: Iterator<Item = U>,
        U: Display,
    {
        self.columns.clear();
        self.columns.reserve(columns.size_hint().0 * 3);

        let mut columns = columns.peekable();

        while let Some(variant) = columns.next() {
            write!(self.columns, "{}", PostgresIdentifier::Simple(variant)).expect("Failure writing to string.");

            if columns.peek().is_some() {
                self.columns.push_str(", ");
            }
        }

        self
    }
}

impl<T: Display> Display for CreateIndex<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "CREATE {uniqueness}INDEX {index_name} ON {table_reference}({columns})",
            uniqueness = if self.is_unique { "UNIQUE " } else { "" },
            index_name = self.index_name,
            table_reference = self.table_reference,
            columns = self.columns,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_enum_without_variants() {
        let create_enum = CreateEnum::named("myEnum");

        assert_eq!(create_enum.to_string(), r#"CREATE TYPE "myEnum" AS ENUM ()"#);
    }

    #[test]
    fn create_enum_with_variants() {
        let variants = &["One", "Two", "Three"];
        let create_enum = CreateEnum::named("myEnum").with_variants(variants.iter());

        assert_eq!(
            create_enum.to_string(),
            r#"CREATE TYPE "myEnum" AS ENUM ('One', 'Two', 'Three')"#
        );
    }

    #[test]
    fn create_unique_index() {
        let columns = &["name", "age"];
        let create_index = CreateIndex::new("meow_idx", true, "Cat").with_columns(columns.iter());

        assert_eq!(
            create_index.to_string(),
            "CREATE UNIQUE INDEX \"meow_idx\" ON \"Cat\"(\"name\", \"age\")"
        )
    }
}
