use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::digest::{IdentityError, SchemaIdV1, SchemaIdentityKindV1, derive_identity};

pub const SCHEMA_DESCRIPTOR_VERSION_V1: u64 = 1;
pub const MAX_SCHEMA_CLOSURE_ITEMS: usize = 4_096;
pub const MAX_SCHEMA_FIELDS: usize = 4_096;
pub const MAX_TYPE_DEPTH: usize = 64;
pub const MAX_TUPLE_ITEMS: usize = 1_024;
pub const MAX_ENUM_VARIANTS: usize = 65_536;
pub const MAX_FIELD_CONSTRAINTS: usize = 128;
pub const MAX_CROSS_CONSTRAINTS: usize = 4_096;
pub const MAX_FIELD_PATH_STEPS: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaDescriptorV1 {
    schema_name: String,
    schema_version: u64,
    fields: Vec<FieldDescriptorV1>,
    cross_constraints: Vec<CrossConstraintExprV1>,
}

impl SchemaDescriptorV1 {
    pub fn new(
        schema_name: impl Into<String>,
        schema_version: u64,
        fields: Vec<FieldDescriptorV1>,
        cross_constraints: Vec<CrossConstraintExprV1>,
    ) -> Result<Self, SchemaError> {
        let descriptor = Self {
            schema_name: schema_name.into(),
            schema_version,
            fields,
            cross_constraints,
        };
        descriptor.validate_local()?;
        Ok(descriptor)
    }

    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub fn schema_version(&self) -> u64 {
        self.schema_version
    }

    pub fn fields(&self) -> &[FieldDescriptorV1] {
        &self.fields
    }

    pub fn cross_constraints(&self) -> &[CrossConstraintExprV1] {
        &self.cross_constraints
    }

    pub fn canonical_value(&self) -> Result<CborValue, SchemaError> {
        self.validate_local()?;
        Ok(CborValue::Array(vec![
            ascii_text(&self.schema_name)?,
            CborValue::Unsigned(self.schema_version),
            CborValue::Array(
                self.fields
                    .iter()
                    .map(FieldDescriptorV1::canonical_value)
                    .collect::<Result<_, _>>()?,
            ),
            CborValue::Array(
                self.cross_constraints
                    .iter()
                    .map(CrossConstraintExprV1::canonical_value)
                    .collect::<Result<_, _>>()?,
            ),
        ]))
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, SchemaError> {
        Ok(deterministic_cbor::encode(&self.canonical_value()?)?)
    }

    pub fn schema_id(&self) -> Result<SchemaIdV1, SchemaError> {
        let mut references = Vec::new();
        for field in &self.fields {
            collect_references(&field.type_expr, &mut references);
        }
        if !references.is_empty() {
            return Err(SchemaError::SchemaReferencesRequireClosure);
        }
        let descriptors = std::slice::from_ref(self);
        let indices = BTreeMap::from([((self.schema_name.clone(), self.schema_version), 0_usize)]);
        validate_semantic_paths(self, 0, descriptors, &indices)?;
        self.compute_schema_id()
    }

    fn validate_local(&self) -> Result<(), SchemaError> {
        validate_ascii_name(&self.schema_name, "schema name")?;
        if self.schema_version != SCHEMA_DESCRIPTOR_VERSION_V1 {
            return Err(SchemaError::UnsupportedSchemaVersion(self.schema_version));
        }
        enforce_limit(self.fields.len(), MAX_SCHEMA_FIELDS, "schema fields")?;
        enforce_limit(
            self.cross_constraints.len(),
            MAX_CROSS_CONSTRAINTS,
            "cross constraints",
        )?;

        let mut names = BTreeSet::new();
        let mut previous_position = None;
        for field in &self.fields {
            field.validate_local()?;
            if let Some(previous) = previous_position
                && field.position <= previous
            {
                return Err(SchemaError::FieldsNotStrictlyPositionSorted);
            }
            previous_position = Some(field.position);
            if !names.insert(field.name.clone()) {
                return Err(SchemaError::DuplicateFieldName(field.name.clone()));
            }
        }

        for constraint in &self.cross_constraints {
            constraint.validate_local()?;
        }
        validate_canonical_order(
            &self.cross_constraints,
            CrossConstraintExprV1::canonical_value,
            SchemaError::CrossConstraintsNotStrictlySorted,
        )?;
        Ok(())
    }

    fn compute_schema_id(&self) -> Result<SchemaIdV1, SchemaError> {
        Ok(derive_identity::<SchemaIdentityKindV1>(
            &self.canonical_value()?,
        )?)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldDescriptorV1 {
    position: u64,
    name: String,
    type_expr: TypeExprV1,
    constraints: Vec<ConstraintExprV1>,
}

impl FieldDescriptorV1 {
    pub fn new(
        position: u64,
        name: impl Into<String>,
        type_expr: TypeExprV1,
        constraints: Vec<ConstraintExprV1>,
    ) -> Result<Self, SchemaError> {
        let field = Self {
            position,
            name: name.into(),
            type_expr,
            constraints,
        };
        field.validate_local()?;
        Ok(field)
    }

    pub fn position(&self) -> u64 {
        self.position
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn type_expr(&self) -> &TypeExprV1 {
        &self.type_expr
    }

    pub fn constraints(&self) -> &[ConstraintExprV1] {
        &self.constraints
    }

    fn validate_local(&self) -> Result<(), SchemaError> {
        if self.position == 0 {
            return Err(SchemaError::ZeroFieldPosition);
        }
        validate_ascii_name(&self.name, "field name")?;
        validate_type_expr(&self.type_expr, 0)?;
        if self.constraints.is_empty() {
            return Err(SchemaError::EmptyConstraintList);
        }
        enforce_limit(
            self.constraints.len(),
            MAX_FIELD_CONSTRAINTS,
            "field constraints",
        )?;
        for constraint in &self.constraints {
            constraint.validate_local(&self.type_expr)?;
        }
        if self.constraints.len() != 1
            && self
                .constraints
                .iter()
                .any(|constraint| matches!(constraint, ConstraintExprV1::NoAdditional))
        {
            return Err(SchemaError::NoAdditionalConstraintMustBeSole);
        }
        validate_canonical_order(
            &self.constraints,
            ConstraintExprV1::canonical_value,
            SchemaError::ConstraintsNotStrictlySorted,
        )
    }

    fn canonical_value(&self) -> Result<CborValue, SchemaError> {
        Ok(CborValue::Array(vec![
            CborValue::Unsigned(self.position),
            ascii_text(&self.name)?,
            self.type_expr.canonical_value()?,
            CborValue::Array(
                self.constraints
                    .iter()
                    .map(ConstraintExprV1::canonical_value)
                    .collect::<Result<_, _>>()?,
            ),
        ]))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeExprV1 {
    Unsigned,
    Boolean,
    AsciiText,
    ExactBytes(u64),
    SchemaReference(SchemaReferenceV1),
    Optional(Box<TypeExprV1>),
    OrderedList(Box<TypeExprV1>),
    Tuple(Vec<TypeExprV1>),
    ClosedEnum {
        enum_name: String,
        variants: Vec<EnumVariantV1>,
    },
}

impl TypeExprV1 {
    pub fn canonical_value(&self) -> Result<CborValue, SchemaError> {
        validate_type_expr(self, 0)?;
        Ok(match self {
            Self::Unsigned => CborValue::Array(vec![CborValue::Unsigned(1)]),
            Self::Boolean => CborValue::Array(vec![CborValue::Unsigned(2)]),
            Self::AsciiText => CborValue::Array(vec![CborValue::Unsigned(3)]),
            Self::ExactBytes(length) => {
                CborValue::Array(vec![CborValue::Unsigned(4), CborValue::Unsigned(*length)])
            }
            Self::SchemaReference(reference) => CborValue::Array(vec![
                CborValue::Unsigned(5),
                ascii_text(reference.schema_name())?,
                CborValue::Unsigned(reference.schema_version()),
                CborValue::Bytes(reference.claimed_schema_id.to_vec()),
            ]),
            Self::Optional(value) => {
                CborValue::Array(vec![CborValue::Unsigned(6), value.canonical_value()?])
            }
            Self::OrderedList(value) => {
                CborValue::Array(vec![CborValue::Unsigned(7), value.canonical_value()?])
            }
            Self::Tuple(values) => CborValue::Array(vec![
                CborValue::Unsigned(8),
                CborValue::Array(
                    values
                        .iter()
                        .map(TypeExprV1::canonical_value)
                        .collect::<Result<_, _>>()?,
                ),
            ]),
            Self::ClosedEnum {
                enum_name,
                variants,
            } => CborValue::Array(vec![
                CborValue::Unsigned(9),
                ascii_text(enum_name)?,
                CborValue::Array(
                    variants
                        .iter()
                        .map(EnumVariantV1::canonical_value)
                        .collect::<Result<_, _>>()?,
                ),
            ]),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaReferenceV1 {
    schema_name: String,
    schema_version: u64,
    claimed_schema_id: [u8; 32],
}

impl SchemaReferenceV1 {
    pub fn claimed(
        schema_name: impl Into<String>,
        schema_version: u64,
        claimed_schema_id: [u8; 32],
    ) -> Result<Self, SchemaError> {
        let reference = Self {
            schema_name: schema_name.into(),
            schema_version,
            claimed_schema_id,
        };
        reference.validate()?;
        Ok(reference)
    }

    pub fn verified(
        schema_name: impl Into<String>,
        schema_version: u64,
        schema_id: &SchemaIdV1,
    ) -> Result<Self, SchemaError> {
        Self::claimed(schema_name, schema_version, *schema_id.as_bytes())
    }

    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub fn schema_version(&self) -> u64 {
        self.schema_version
    }

    pub fn claimed_schema_id(&self) -> &[u8; 32] {
        &self.claimed_schema_id
    }

    fn validate(&self) -> Result<(), SchemaError> {
        validate_ascii_name(&self.schema_name, "referenced schema name")?;
        if self.schema_version != SCHEMA_DESCRIPTOR_VERSION_V1 {
            return Err(SchemaError::UnsupportedSchemaVersion(self.schema_version));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumVariantV1 {
    tag: u64,
    name: String,
}

impl EnumVariantV1 {
    pub fn new(tag: u64, name: impl Into<String>) -> Result<Self, SchemaError> {
        let variant = Self {
            tag,
            name: name.into(),
        };
        if variant.tag == 0 {
            return Err(SchemaError::ZeroEnumTag);
        }
        validate_ascii_name(&variant.name, "enum variant name")?;
        Ok(variant)
    }

    pub fn tag(&self) -> u64 {
        self.tag
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    fn canonical_value(&self) -> Result<CborValue, SchemaError> {
        Ok(CborValue::Array(vec![
            CborValue::Unsigned(self.tag),
            ascii_text(&self.name)?,
        ]))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConstraintExprV1 {
    NoAdditional,
    BoundedLength {
        minimum: u64,
        maximum: u64,
    },
    CanonicalSet {
        key_path: FieldPathV1,
        minimum: u64,
        maximum: u64,
    },
    UnsignedRange {
        minimum: u64,
        maximum: u64,
    },
    ExactFieldEquality(FieldPathV1),
}

impl ConstraintExprV1 {
    pub fn canonical_value(&self) -> Result<CborValue, SchemaError> {
        Ok(match self {
            Self::NoAdditional => CborValue::Array(vec![CborValue::Unsigned(1)]),
            Self::BoundedLength { minimum, maximum } => CborValue::Array(vec![
                CborValue::Unsigned(2),
                CborValue::Unsigned(*minimum),
                CborValue::Unsigned(*maximum),
            ]),
            Self::CanonicalSet {
                key_path,
                minimum,
                maximum,
            } => CborValue::Array(vec![
                CborValue::Unsigned(3),
                key_path.canonical_value()?,
                CborValue::Unsigned(*minimum),
                CborValue::Unsigned(*maximum),
            ]),
            Self::UnsignedRange { minimum, maximum } => CborValue::Array(vec![
                CborValue::Unsigned(4),
                CborValue::Unsigned(*minimum),
                CborValue::Unsigned(*maximum),
            ]),
            Self::ExactFieldEquality(path) => {
                CborValue::Array(vec![CborValue::Unsigned(5), path.canonical_value()?])
            }
        })
    }

    fn validate_local(&self, field_type: &TypeExprV1) -> Result<(), SchemaError> {
        match self {
            Self::NoAdditional => Ok(()),
            Self::BoundedLength { minimum, maximum } => {
                validate_range(*minimum, *maximum)?;
                if matches!(
                    field_type,
                    TypeExprV1::AsciiText
                        | TypeExprV1::ExactBytes(_)
                        | TypeExprV1::OrderedList(_)
                        | TypeExprV1::Tuple(_)
                ) {
                    Ok(())
                } else {
                    Err(SchemaError::ConstraintNotApplicable)
                }
            }
            Self::CanonicalSet {
                key_path,
                minimum,
                maximum,
            } => {
                key_path.validate_local()?;
                validate_range(*minimum, *maximum)?;
                if matches!(field_type, TypeExprV1::OrderedList(_)) {
                    Ok(())
                } else {
                    Err(SchemaError::ConstraintNotApplicable)
                }
            }
            Self::UnsignedRange { minimum, maximum } => {
                validate_range(*minimum, *maximum)?;
                if matches!(field_type, TypeExprV1::Unsigned) {
                    Ok(())
                } else {
                    Err(SchemaError::ConstraintNotApplicable)
                }
            }
            Self::ExactFieldEquality(path) => path.validate_local(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CrossConstraintExprV1 {
    Equality {
        left: FieldPathV1,
        right: FieldPathV1,
    },
    ExactlyOnePresent(Vec<FieldPathV1>),
    AllPresentOrAllAbsent(Vec<FieldPathV1>),
}

impl CrossConstraintExprV1 {
    pub fn canonical_value(&self) -> Result<CborValue, SchemaError> {
        Ok(match self {
            Self::Equality { left, right } => CborValue::Array(vec![
                CborValue::Unsigned(1),
                left.canonical_value()?,
                right.canonical_value()?,
            ]),
            Self::ExactlyOnePresent(paths) => CborValue::Array(vec![
                CborValue::Unsigned(2),
                CborValue::Array(
                    paths
                        .iter()
                        .map(FieldPathV1::canonical_value)
                        .collect::<Result<_, _>>()?,
                ),
            ]),
            Self::AllPresentOrAllAbsent(paths) => CborValue::Array(vec![
                CborValue::Unsigned(3),
                CborValue::Array(
                    paths
                        .iter()
                        .map(FieldPathV1::canonical_value)
                        .collect::<Result<_, _>>()?,
                ),
            ]),
        })
    }

    fn validate_local(&self) -> Result<(), SchemaError> {
        match self {
            Self::Equality { left, right } => {
                left.validate_local()?;
                right.validate_local()
            }
            Self::ExactlyOnePresent(paths) | Self::AllPresentOrAllAbsent(paths) => {
                if paths.is_empty() {
                    return Err(SchemaError::EmptyFieldPathSet);
                }
                for path in paths {
                    path.validate_local()?;
                }
                validate_canonical_order(
                    paths,
                    FieldPathV1::canonical_value,
                    SchemaError::FieldPathsNotStrictlySorted,
                )
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldPathV1(Vec<PathStepV1>);

impl FieldPathV1 {
    pub fn new(steps: Vec<PathStepV1>) -> Result<Self, SchemaError> {
        let path = Self(steps);
        path.validate_local()?;
        Ok(path)
    }

    pub fn steps(&self) -> &[PathStepV1] {
        &self.0
    }

    pub fn canonical_value(&self) -> Result<CborValue, SchemaError> {
        self.validate_local()?;
        Ok(CborValue::Array(
            self.0.iter().map(PathStepV1::canonical_value).collect(),
        ))
    }

    fn validate_local(&self) -> Result<(), SchemaError> {
        if self.0.is_empty() {
            return Err(SchemaError::EmptyFieldPath);
        }
        enforce_limit(self.0.len(), MAX_FIELD_PATH_STEPS, "field path steps")?;
        for step in &self.0 {
            if matches!(step, PathStepV1::Field(0)) {
                return Err(SchemaError::ZeroFieldPosition);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PathStepV1 {
    Field(u64),
    TupleIndex(u64),
}

impl PathStepV1 {
    fn canonical_value(&self) -> CborValue {
        match self {
            Self::Field(position) => {
                CborValue::Array(vec![CborValue::Unsigned(1), CborValue::Unsigned(*position)])
            }
            Self::TupleIndex(index) => {
                CborValue::Array(vec![CborValue::Unsigned(2), CborValue::Unsigned(*index)])
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct SchemaClosureV1 {
    descriptors: Vec<SchemaDescriptorV1>,
    schema_ids: Vec<SchemaIdV1>,
}

impl SchemaClosureV1 {
    pub fn new(descriptors: Vec<SchemaDescriptorV1>) -> Result<Self, SchemaError> {
        if descriptors.is_empty() {
            return Err(SchemaError::EmptySchemaClosure);
        }
        enforce_limit(
            descriptors.len(),
            MAX_SCHEMA_CLOSURE_ITEMS,
            "schema closure",
        )?;

        let mut indices = BTreeMap::new();
        for (index, descriptor) in descriptors.iter().enumerate() {
            descriptor.validate_local()?;
            let key = (descriptor.schema_name.clone(), descriptor.schema_version);
            if indices.insert(key.clone(), index).is_some() {
                return Err(SchemaError::DuplicateSchemaKey {
                    name: key.0,
                    version: key.1,
                });
            }
        }

        let mut schema_ids = Vec::with_capacity(descriptors.len());
        for (index, descriptor) in descriptors.iter().enumerate() {
            validate_references(descriptor, index, &descriptors, &indices, &schema_ids)?;
            validate_semantic_paths(descriptor, index, &descriptors, &indices)?;
            schema_ids.push(descriptor.compute_schema_id()?);
        }

        Ok(Self {
            descriptors,
            schema_ids,
        })
    }

    pub fn descriptors(&self) -> &[SchemaDescriptorV1] {
        &self.descriptors
    }

    pub fn schema_ids(&self) -> &[SchemaIdV1] {
        &self.schema_ids
    }

    pub fn schema_id(&self, name: &str, version: u64) -> Option<&SchemaIdV1> {
        self.descriptors
            .iter()
            .position(|descriptor| {
                descriptor.schema_name == name && descriptor.schema_version == version
            })
            .map(|index| &self.schema_ids[index])
    }

    pub fn descriptor_for_id(&self, schema_id: &SchemaIdV1) -> Option<&SchemaDescriptorV1> {
        self.schema_ids
            .iter()
            .position(|candidate| candidate == schema_id)
            .map(|index| &self.descriptors[index])
    }

    pub fn validate_value(
        &self,
        schema_id: &SchemaIdV1,
        value: &CborValue,
    ) -> Result<(), SchemaError> {
        deterministic_cbor::encode(value)?;
        let index = self
            .schema_ids
            .iter()
            .position(|candidate| candidate == schema_id)
            .ok_or(SchemaError::UnknownSchemaId)?;
        validate_descriptor_value(self, index, value)
    }
}

pub fn optional_value_v1(value: Option<CborValue>) -> CborValue {
    CborValue::optional(value)
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SchemaError {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error("{0} must be nonempty ASCII text")]
    InvalidAsciiName(&'static str),
    #[error("schema version {0} is unsupported by SchemaDescriptorV1")]
    UnsupportedSchemaVersion(u64),
    #[error("{0} exceeds its finite v1 limit")]
    LimitExceeded(&'static str),
    #[error("field position must be positive")]
    ZeroFieldPosition,
    #[error("schema fields are not strictly sorted by positive position")]
    FieldsNotStrictlyPositionSorted,
    #[error("duplicate field name {0}")]
    DuplicateFieldName(String),
    #[error("enum tag must be positive")]
    ZeroEnumTag,
    #[error("enum variants are not strictly sorted by positive tag")]
    EnumTagsNotStrictlySorted,
    #[error("duplicate enum variant name {0}")]
    DuplicateEnumName(String),
    #[error("tuple and enum variants must be nonempty")]
    EmptyTypeMembers,
    #[error("type expression exceeds its finite v1 nesting limit")]
    TypeNestingTooDeep,
    #[error("every field must declare a nonempty constraint list")]
    EmptyConstraintList,
    #[error("constraint [1] is valid only as the sole constraint")]
    NoAdditionalConstraintMustBeSole,
    #[error("constraint minimum exceeds maximum")]
    InvalidRange,
    #[error("constraint does not apply to the field type")]
    ConstraintNotApplicable,
    #[error("field constraints are not strictly sorted by canonical bytes")]
    ConstraintsNotStrictlySorted,
    #[error("cross constraints are not strictly sorted by canonical bytes")]
    CrossConstraintsNotStrictlySorted,
    #[error("FieldPathV1 must be nonempty")]
    EmptyFieldPath,
    #[error("a cross-constraint field-path set must be nonempty")]
    EmptyFieldPathSet,
    #[error("field paths are not strictly sorted by canonical bytes")]
    FieldPathsNotStrictlySorted,
    #[error("field path is invalid for the traversed type")]
    InvalidFieldPath,
    #[error("field path cannot traverse an optional value without presence")]
    OptionalTraversal,
    #[error("canonical-set key path must terminate at an unsigned integer")]
    CanonicalSetKeyMustBeUnsigned,
    #[error("cross-constraint presence paths must terminate at optional fields")]
    PresencePathMustBeOptional,
    #[error("equality constraint paths must terminate at equal types")]
    EqualityTypeMismatch,
    #[error("schema closure must be nonempty")]
    EmptySchemaClosure,
    #[error("schema references must be validated and identified through SchemaClosureV1")]
    SchemaReferencesRequireClosure,
    #[error("duplicate schema key {name}@{version}")]
    DuplicateSchemaKey { name: String, version: u64 },
    #[error("unknown schema reference {name}@{version}")]
    UnknownSchemaReference { name: String, version: u64 },
    #[error("schema reference {name}@{version} is forward or cyclic")]
    ForwardOrCyclicSchemaReference { name: String, version: u64 },
    #[error("schema reference identity does not match recomputed {name}@{version}")]
    ReferencedSchemaIdMismatch { name: String, version: u64 },
    #[error("SchemaIdV1 is not present in the immutable SchemaClosureV1")]
    UnknownSchemaId,
    #[error("schema value must be one definite array with exactly one item per declared field")]
    SchemaValueShapeMismatch,
    #[error("schema value does not match the declared TypeExprV1")]
    ValueTypeMismatch,
    #[error("schema value violates an exact byte-length constraint")]
    ExactBytesLengthMismatch,
    #[error("schema value violates a bounded-length constraint")]
    BoundedLengthViolation,
    #[error("schema value violates an unsigned-range constraint")]
    UnsignedRangeViolation,
    #[error("schema value contains an unknown closed-enum tag")]
    UnknownEnumTag,
    #[error("canonical-set values are not strictly sorted by their unsigned key")]
    CanonicalSetValuesNotStrictlySorted,
    #[error("schema value violates an exact field-equality constraint")]
    FieldEqualityViolation,
    #[error("schema value violates an exactly-one-present constraint")]
    ExactlyOnePresentViolation,
    #[error("schema value violates an all-present-or-all-absent constraint")]
    AllPresentOrAllAbsentViolation,
}

fn validate_type_expr(value: &TypeExprV1, depth: usize) -> Result<(), SchemaError> {
    if depth > MAX_TYPE_DEPTH {
        return Err(SchemaError::TypeNestingTooDeep);
    }
    match value {
        TypeExprV1::Unsigned
        | TypeExprV1::Boolean
        | TypeExprV1::AsciiText
        | TypeExprV1::ExactBytes(_) => Ok(()),
        TypeExprV1::SchemaReference(reference) => reference.validate(),
        TypeExprV1::Optional(value) | TypeExprV1::OrderedList(value) => {
            validate_type_expr(value, depth + 1)
        }
        TypeExprV1::Tuple(values) => {
            if values.is_empty() {
                return Err(SchemaError::EmptyTypeMembers);
            }
            enforce_limit(values.len(), MAX_TUPLE_ITEMS, "tuple items")?;
            for value in values {
                validate_type_expr(value, depth + 1)?;
            }
            Ok(())
        }
        TypeExprV1::ClosedEnum {
            enum_name,
            variants,
        } => {
            validate_ascii_name(enum_name, "enum name")?;
            if variants.is_empty() {
                return Err(SchemaError::EmptyTypeMembers);
            }
            enforce_limit(variants.len(), MAX_ENUM_VARIANTS, "enum variants")?;
            let mut names = BTreeSet::new();
            let mut previous_tag = None;
            for variant in variants {
                if variant.tag == 0 {
                    return Err(SchemaError::ZeroEnumTag);
                }
                validate_ascii_name(&variant.name, "enum variant name")?;
                if let Some(previous) = previous_tag
                    && variant.tag <= previous
                {
                    return Err(SchemaError::EnumTagsNotStrictlySorted);
                }
                previous_tag = Some(variant.tag);
                if !names.insert(variant.name.clone()) {
                    return Err(SchemaError::DuplicateEnumName(variant.name.clone()));
                }
            }
            Ok(())
        }
    }
}

fn validate_references(
    descriptor: &SchemaDescriptorV1,
    current_index: usize,
    descriptors: &[SchemaDescriptorV1],
    indices: &BTreeMap<(String, u64), usize>,
    schema_ids: &[SchemaIdV1],
) -> Result<(), SchemaError> {
    let mut references = Vec::new();
    for field in &descriptor.fields {
        collect_references(&field.type_expr, &mut references);
    }
    for reference in references {
        let key = (reference.schema_name.clone(), reference.schema_version);
        let Some(target_index) = indices.get(&key).copied() else {
            return Err(SchemaError::UnknownSchemaReference {
                name: key.0,
                version: key.1,
            });
        };
        if target_index >= current_index {
            return Err(SchemaError::ForwardOrCyclicSchemaReference {
                name: key.0,
                version: key.1,
            });
        }
        let actual = schema_ids.get(target_index).ok_or_else(|| {
            SchemaError::ForwardOrCyclicSchemaReference {
                name: key.0.clone(),
                version: key.1,
            }
        })?;
        if actual.as_bytes() != &reference.claimed_schema_id {
            return Err(SchemaError::ReferencedSchemaIdMismatch {
                name: key.0,
                version: key.1,
            });
        }
        let target = &descriptors[target_index];
        if target.schema_name != reference.schema_name
            || target.schema_version != reference.schema_version
        {
            return Err(SchemaError::ReferencedSchemaIdMismatch {
                name: reference.schema_name.clone(),
                version: reference.schema_version,
            });
        }
    }
    Ok(())
}

fn collect_references<'a>(value: &'a TypeExprV1, output: &mut Vec<&'a SchemaReferenceV1>) {
    match value {
        TypeExprV1::SchemaReference(reference) => output.push(reference),
        TypeExprV1::Optional(value) | TypeExprV1::OrderedList(value) => {
            collect_references(value, output);
        }
        TypeExprV1::Tuple(values) => {
            for value in values {
                collect_references(value, output);
            }
        }
        TypeExprV1::Unsigned
        | TypeExprV1::Boolean
        | TypeExprV1::AsciiText
        | TypeExprV1::ExactBytes(_)
        | TypeExprV1::ClosedEnum { .. } => {}
    }
}

fn validate_semantic_paths(
    descriptor: &SchemaDescriptorV1,
    current_index: usize,
    descriptors: &[SchemaDescriptorV1],
    indices: &BTreeMap<(String, u64), usize>,
) -> Result<(), SchemaError> {
    for field in &descriptor.fields {
        for constraint in &field.constraints {
            match constraint {
                ConstraintExprV1::CanonicalSet { key_path, .. } => {
                    let TypeExprV1::OrderedList(element_type) = &field.type_expr else {
                        return Err(SchemaError::ConstraintNotApplicable);
                    };
                    let terminal = resolve_path(
                        PathCursor::Type(element_type),
                        key_path,
                        current_index,
                        descriptors,
                        indices,
                    )?;
                    if !matches!(terminal, TypeExprV1::Unsigned) {
                        return Err(SchemaError::CanonicalSetKeyMustBeUnsigned);
                    }
                }
                ConstraintExprV1::ExactFieldEquality(path) => {
                    let terminal = resolve_path(
                        PathCursor::Schema(descriptor),
                        path,
                        current_index,
                        descriptors,
                        indices,
                    )?;
                    if terminal != &field.type_expr {
                        return Err(SchemaError::EqualityTypeMismatch);
                    }
                }
                ConstraintExprV1::NoAdditional
                | ConstraintExprV1::BoundedLength { .. }
                | ConstraintExprV1::UnsignedRange { .. } => {}
            }
        }
    }

    for constraint in &descriptor.cross_constraints {
        constraint.validate_local()?;
        match constraint {
            CrossConstraintExprV1::Equality { left, right } => {
                let left_type = resolve_path(
                    PathCursor::Schema(descriptor),
                    left,
                    current_index,
                    descriptors,
                    indices,
                )?;
                let right_type = resolve_path(
                    PathCursor::Schema(descriptor),
                    right,
                    current_index,
                    descriptors,
                    indices,
                )?;
                if left_type != right_type {
                    return Err(SchemaError::EqualityTypeMismatch);
                }
            }
            CrossConstraintExprV1::ExactlyOnePresent(paths)
            | CrossConstraintExprV1::AllPresentOrAllAbsent(paths) => {
                for path in paths {
                    let terminal = resolve_path(
                        PathCursor::Schema(descriptor),
                        path,
                        current_index,
                        descriptors,
                        indices,
                    )?;
                    if !matches!(terminal, TypeExprV1::Optional(_)) {
                        return Err(SchemaError::PresencePathMustBeOptional);
                    }
                }
            }
        }
    }
    Ok(())
}

enum PathCursor<'a> {
    Schema(&'a SchemaDescriptorV1),
    Type(&'a TypeExprV1),
}

fn resolve_path<'a>(
    mut cursor: PathCursor<'a>,
    path: &FieldPathV1,
    current_index: usize,
    descriptors: &'a [SchemaDescriptorV1],
    indices: &BTreeMap<(String, u64), usize>,
) -> Result<&'a TypeExprV1, SchemaError> {
    for step in path.steps() {
        cursor = match (cursor, step) {
            (PathCursor::Schema(schema), PathStepV1::Field(position)) => {
                let field = schema
                    .fields
                    .iter()
                    .find(|field| field.position == *position)
                    .ok_or(SchemaError::InvalidFieldPath)?;
                PathCursor::Type(&field.type_expr)
            }
            (PathCursor::Type(TypeExprV1::Tuple(values)), PathStepV1::TupleIndex(index)) => {
                let index = usize::try_from(*index).map_err(|_| SchemaError::InvalidFieldPath)?;
                PathCursor::Type(values.get(index).ok_or(SchemaError::InvalidFieldPath)?)
            }
            (
                PathCursor::Type(TypeExprV1::SchemaReference(reference)),
                PathStepV1::Field(position),
            ) => {
                let key = (reference.schema_name.clone(), reference.schema_version);
                let target_index = indices.get(&key).copied().ok_or_else(|| {
                    SchemaError::UnknownSchemaReference {
                        name: key.0.clone(),
                        version: key.1,
                    }
                })?;
                if target_index >= current_index {
                    return Err(SchemaError::ForwardOrCyclicSchemaReference {
                        name: key.0,
                        version: key.1,
                    });
                }
                let field = descriptors[target_index]
                    .fields
                    .iter()
                    .find(|field| field.position == *position)
                    .ok_or(SchemaError::InvalidFieldPath)?;
                PathCursor::Type(&field.type_expr)
            }
            (PathCursor::Type(TypeExprV1::Optional(_)), _) => {
                return Err(SchemaError::OptionalTraversal);
            }
            _ => return Err(SchemaError::InvalidFieldPath),
        };
    }
    match cursor {
        PathCursor::Type(value) => Ok(value),
        PathCursor::Schema(_) => Err(SchemaError::InvalidFieldPath),
    }
}

fn validate_descriptor_value(
    closure: &SchemaClosureV1,
    descriptor_index: usize,
    value: &CborValue,
) -> Result<(), SchemaError> {
    let descriptor = &closure.descriptors[descriptor_index];
    let CborValue::Array(field_values) = value else {
        return Err(SchemaError::SchemaValueShapeMismatch);
    };
    if field_values.len() != descriptor.fields.len() {
        return Err(SchemaError::SchemaValueShapeMismatch);
    }

    for (field, field_value) in descriptor.fields.iter().zip(field_values) {
        validate_type_value(closure, descriptor_index, &field.type_expr, field_value)?;
        validate_field_constraints(
            closure,
            descriptor_index,
            descriptor,
            value,
            field,
            field_value,
        )?;
    }
    validate_cross_constraint_values(closure, descriptor_index, descriptor, value)
}

fn validate_type_value(
    closure: &SchemaClosureV1,
    descriptor_index: usize,
    type_expr: &TypeExprV1,
    value: &CborValue,
) -> Result<(), SchemaError> {
    match (type_expr, value) {
        (TypeExprV1::Unsigned, CborValue::Unsigned(_))
        | (TypeExprV1::Boolean, CborValue::Bool(_)) => Ok(()),
        (TypeExprV1::AsciiText, CborValue::Text(value)) if value.is_ascii() => Ok(()),
        (TypeExprV1::ExactBytes(expected), CborValue::Bytes(value)) => {
            if u64::try_from(value.len()).ok() == Some(*expected) {
                Ok(())
            } else {
                Err(SchemaError::ExactBytesLengthMismatch)
            }
        }
        (TypeExprV1::SchemaReference(reference), value) => {
            let target_index = closure
                .descriptors
                .iter()
                .position(|descriptor| {
                    descriptor.schema_name == reference.schema_name
                        && descriptor.schema_version == reference.schema_version
                })
                .ok_or_else(|| SchemaError::UnknownSchemaReference {
                    name: reference.schema_name.clone(),
                    version: reference.schema_version,
                })?;
            if target_index >= descriptor_index {
                return Err(SchemaError::ForwardOrCyclicSchemaReference {
                    name: reference.schema_name.clone(),
                    version: reference.schema_version,
                });
            }
            if closure.schema_ids[target_index].as_bytes() != &reference.claimed_schema_id {
                return Err(SchemaError::ReferencedSchemaIdMismatch {
                    name: reference.schema_name.clone(),
                    version: reference.schema_version,
                });
            }
            validate_descriptor_value(closure, target_index, value)
        }
        (TypeExprV1::Optional(inner), CborValue::Array(values)) => match values.as_slice() {
            [CborValue::Unsigned(0)] => Ok(()),
            [CborValue::Unsigned(1), value] => {
                validate_type_value(closure, descriptor_index, inner, value)
            }
            _ => Err(SchemaError::ValueTypeMismatch),
        },
        (TypeExprV1::OrderedList(inner), CborValue::Array(values)) => {
            for value in values {
                validate_type_value(closure, descriptor_index, inner, value)?;
            }
            Ok(())
        }
        (TypeExprV1::Tuple(types), CborValue::Array(values)) if types.len() == values.len() => {
            for (type_expr, value) in types.iter().zip(values) {
                validate_type_value(closure, descriptor_index, type_expr, value)?;
            }
            Ok(())
        }
        (TypeExprV1::ClosedEnum { variants, .. }, CborValue::Unsigned(selected_tag)) => {
            if variants.iter().any(|variant| variant.tag == *selected_tag) {
                Ok(())
            } else {
                Err(SchemaError::UnknownEnumTag)
            }
        }
        _ => Err(SchemaError::ValueTypeMismatch),
    }
}

fn validate_field_constraints(
    closure: &SchemaClosureV1,
    descriptor_index: usize,
    descriptor: &SchemaDescriptorV1,
    descriptor_value: &CborValue,
    field: &FieldDescriptorV1,
    field_value: &CborValue,
) -> Result<(), SchemaError> {
    for constraint in &field.constraints {
        match constraint {
            ConstraintExprV1::NoAdditional => {}
            ConstraintExprV1::BoundedLength { minimum, maximum } => {
                let length = value_length(field_value).ok_or(SchemaError::ValueTypeMismatch)?;
                if length < *minimum || length > *maximum {
                    return Err(SchemaError::BoundedLengthViolation);
                }
            }
            ConstraintExprV1::CanonicalSet {
                key_path,
                minimum,
                maximum,
            } => {
                let TypeExprV1::OrderedList(element_type) = &field.type_expr else {
                    return Err(SchemaError::ConstraintNotApplicable);
                };
                let CborValue::Array(elements) = field_value else {
                    return Err(SchemaError::ValueTypeMismatch);
                };
                let length = u64::try_from(elements.len())
                    .map_err(|_| SchemaError::BoundedLengthViolation)?;
                if length < *minimum || length > *maximum {
                    return Err(SchemaError::BoundedLengthViolation);
                }
                let mut previous_key = None;
                for element in elements {
                    let (terminal_type, terminal_value) = resolve_value_path(
                        closure,
                        descriptor_index,
                        ValuePathCursor::Type(element_type, element),
                        key_path,
                    )?;
                    if !matches!(terminal_type, TypeExprV1::Unsigned) {
                        return Err(SchemaError::CanonicalSetKeyMustBeUnsigned);
                    }
                    let CborValue::Unsigned(key) = terminal_value else {
                        return Err(SchemaError::ValueTypeMismatch);
                    };
                    if previous_key.is_some_and(|previous| previous >= *key) {
                        return Err(SchemaError::CanonicalSetValuesNotStrictlySorted);
                    }
                    previous_key = Some(*key);
                }
            }
            ConstraintExprV1::UnsignedRange { minimum, maximum } => {
                let CborValue::Unsigned(value) = field_value else {
                    return Err(SchemaError::ValueTypeMismatch);
                };
                if value < minimum || value > maximum {
                    return Err(SchemaError::UnsignedRangeViolation);
                }
            }
            ConstraintExprV1::ExactFieldEquality(path) => {
                let (_, other_value) = resolve_value_path(
                    closure,
                    descriptor_index,
                    ValuePathCursor::Schema(descriptor, descriptor_value),
                    path,
                )?;
                if field_value != other_value {
                    return Err(SchemaError::FieldEqualityViolation);
                }
            }
        }
    }
    Ok(())
}

fn validate_cross_constraint_values(
    closure: &SchemaClosureV1,
    descriptor_index: usize,
    descriptor: &SchemaDescriptorV1,
    descriptor_value: &CborValue,
) -> Result<(), SchemaError> {
    for constraint in &descriptor.cross_constraints {
        match constraint {
            CrossConstraintExprV1::Equality { left, right } => {
                let (_, left_value) = resolve_value_path(
                    closure,
                    descriptor_index,
                    ValuePathCursor::Schema(descriptor, descriptor_value),
                    left,
                )?;
                let (_, right_value) = resolve_value_path(
                    closure,
                    descriptor_index,
                    ValuePathCursor::Schema(descriptor, descriptor_value),
                    right,
                )?;
                if left_value != right_value {
                    return Err(SchemaError::FieldEqualityViolation);
                }
            }
            CrossConstraintExprV1::ExactlyOnePresent(paths) => {
                let present = paths
                    .iter()
                    .map(|path| {
                        resolve_value_path(
                            closure,
                            descriptor_index,
                            ValuePathCursor::Schema(descriptor, descriptor_value),
                            path,
                        )
                        .and_then(|(_, value)| optional_is_present(value))
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .filter(|present| *present)
                    .count();
                if present != 1 {
                    return Err(SchemaError::ExactlyOnePresentViolation);
                }
            }
            CrossConstraintExprV1::AllPresentOrAllAbsent(paths) => {
                let presence = paths
                    .iter()
                    .map(|path| {
                        resolve_value_path(
                            closure,
                            descriptor_index,
                            ValuePathCursor::Schema(descriptor, descriptor_value),
                            path,
                        )
                        .and_then(|(_, value)| optional_is_present(value))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if presence.windows(2).any(|pair| pair[0] != pair[1]) {
                    return Err(SchemaError::AllPresentOrAllAbsentViolation);
                }
            }
        }
    }
    Ok(())
}

enum ValuePathCursor<'schema, 'value> {
    Schema(&'schema SchemaDescriptorV1, &'value CborValue),
    Type(&'schema TypeExprV1, &'value CborValue),
}

fn resolve_value_path<'schema, 'value>(
    closure: &'schema SchemaClosureV1,
    current_index: usize,
    mut cursor: ValuePathCursor<'schema, 'value>,
    path: &FieldPathV1,
) -> Result<(&'schema TypeExprV1, &'value CborValue), SchemaError> {
    for step in path.steps() {
        cursor = match (cursor, step) {
            (ValuePathCursor::Schema(schema, value), PathStepV1::Field(position)) => {
                let CborValue::Array(values) = value else {
                    return Err(SchemaError::SchemaValueShapeMismatch);
                };
                let field_index = schema
                    .fields
                    .iter()
                    .position(|field| field.position == *position)
                    .ok_or(SchemaError::InvalidFieldPath)?;
                ValuePathCursor::Type(&schema.fields[field_index].type_expr, &values[field_index])
            }
            (
                ValuePathCursor::Type(TypeExprV1::Tuple(types), value),
                PathStepV1::TupleIndex(index),
            ) => {
                let CborValue::Array(values) = value else {
                    return Err(SchemaError::ValueTypeMismatch);
                };
                let index = usize::try_from(*index).map_err(|_| SchemaError::InvalidFieldPath)?;
                ValuePathCursor::Type(
                    types.get(index).ok_or(SchemaError::InvalidFieldPath)?,
                    values.get(index).ok_or(SchemaError::InvalidFieldPath)?,
                )
            }
            (
                ValuePathCursor::Type(TypeExprV1::SchemaReference(reference), value),
                PathStepV1::Field(position),
            ) => {
                let target_index = closure
                    .descriptors
                    .iter()
                    .position(|descriptor| {
                        descriptor.schema_name == reference.schema_name
                            && descriptor.schema_version == reference.schema_version
                    })
                    .ok_or_else(|| SchemaError::UnknownSchemaReference {
                        name: reference.schema_name.clone(),
                        version: reference.schema_version,
                    })?;
                if target_index >= current_index {
                    return Err(SchemaError::ForwardOrCyclicSchemaReference {
                        name: reference.schema_name.clone(),
                        version: reference.schema_version,
                    });
                }
                let schema = &closure.descriptors[target_index];
                let CborValue::Array(values) = value else {
                    return Err(SchemaError::SchemaValueShapeMismatch);
                };
                let field_index = schema
                    .fields
                    .iter()
                    .position(|field| field.position == *position)
                    .ok_or(SchemaError::InvalidFieldPath)?;
                ValuePathCursor::Type(&schema.fields[field_index].type_expr, &values[field_index])
            }
            (ValuePathCursor::Type(TypeExprV1::Optional(_), _), _) => {
                return Err(SchemaError::OptionalTraversal);
            }
            _ => return Err(SchemaError::InvalidFieldPath),
        };
    }
    match cursor {
        ValuePathCursor::Type(type_expr, value) => Ok((type_expr, value)),
        ValuePathCursor::Schema(_, _) => Err(SchemaError::InvalidFieldPath),
    }
}

fn value_length(value: &CborValue) -> Option<u64> {
    match value {
        CborValue::Bytes(value) => u64::try_from(value.len()).ok(),
        CborValue::Text(value) => u64::try_from(value.len()).ok(),
        CborValue::Array(value) => u64::try_from(value.len()).ok(),
        CborValue::Unsigned(_) | CborValue::Bool(_) => None,
    }
}

fn optional_is_present(value: &CborValue) -> Result<bool, SchemaError> {
    match value {
        CborValue::Array(values) if values.as_slice() == [CborValue::Unsigned(0)] => Ok(false),
        CborValue::Array(values) if matches!(values.as_slice(), [CborValue::Unsigned(1), _]) => {
            Ok(true)
        }
        _ => Err(SchemaError::ValueTypeMismatch),
    }
}

fn validate_canonical_order<T>(
    values: &[T],
    canonical_value: fn(&T) -> Result<CborValue, SchemaError>,
    error: SchemaError,
) -> Result<(), SchemaError> {
    let mut previous = None;
    for value in values {
        let encoded = deterministic_cbor::encode(&canonical_value(value)?)?;
        if previous
            .as_ref()
            .is_some_and(|previous| previous >= &encoded)
        {
            return Err(error);
        }
        previous = Some(encoded);
    }
    Ok(())
}

fn validate_range(minimum: u64, maximum: u64) -> Result<(), SchemaError> {
    if minimum <= maximum {
        Ok(())
    } else {
        Err(SchemaError::InvalidRange)
    }
}

fn validate_ascii_name(value: &str, label: &'static str) -> Result<(), SchemaError> {
    if !value.is_empty() && value.is_ascii() {
        Ok(())
    } else {
        Err(SchemaError::InvalidAsciiName(label))
    }
}

fn ascii_text(value: &str) -> Result<CborValue, SchemaError> {
    Ok(CborValue::text(value.to_owned())?)
}

fn enforce_limit(value: usize, limit: usize, label: &'static str) -> Result<(), SchemaError> {
    if value <= limit {
        Ok(())
    } else {
        Err(SchemaError::LimitExceeded(label))
    }
}
