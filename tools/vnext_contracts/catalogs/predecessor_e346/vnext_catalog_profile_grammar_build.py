#!/usr/bin/env python3
"""Build the design-only CatalogProfileGrammarV1 candidate and byte appendix."""

from __future__ import annotations

import hashlib
import json
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parent
SOURCE_PATH = ROOT / "vnext-catalog-nominal-source-v1.json"
SOURCE = json.loads(SOURCE_PATH.read_text(encoding="ascii"))


def head(major: int, value: int) -> bytes:
    if not 0 <= value <= 0xFFFFFFFFFFFFFFFF:
        raise ValueError("unsigned value outside u64")
    if value < 24:
        return bytes([(major << 5) | value])
    if value <= 0xFF:
        return bytes([(major << 5) | 24, value])
    if value <= 0xFFFF:
        return bytes([(major << 5) | 25]) + value.to_bytes(2, "big")
    if value <= 0xFFFFFFFF:
        return bytes([(major << 5) | 26]) + value.to_bytes(4, "big")
    return bytes([(major << 5) | 27]) + value.to_bytes(8, "big")


def encode(value) -> bytes:
    if value is False:
        return b"\xf4"
    if value is True:
        return b"\xf5"
    if isinstance(value, int) and not isinstance(value, bool):
        return head(0, value)
    if isinstance(value, str):
        raw = value.encode("ascii")
        return head(3, len(raw)) + raw
    if isinstance(value, list):
        return head(4, len(value)) + b"".join(encode(item) for item in value)
    if isinstance(value, dict) and list(value) == ["bytes"]:
        raw = bytes.fromhex(value["bytes"])
        return head(2, len(raw)) + raw
    raise ValueError(f"unsupported value: {value!r}")


def digest(value) -> tuple[str, str, int]:
    raw = encode(value)
    return hashlib.sha256(raw).hexdigest(), raw.hex(), len(raw)


def b32(hex_value: str) -> dict[str, str]:
    if len(hex_value) != 64:
        raise ValueError(hex_value)
    return {"bytes": hex_value}


U64 = [1]
BOOL = [2]
ASCII = [3]
BYTES32 = [4, 32]


def optional(value):
    return [6, value]


def ordered_list(value):
    return [7, value]


def tuple_of(*values):
    return [8, list(values)]


def enum(enum_id: str, rows: list[list]):
    return [9, enum_id, rows]


NO_EXTRA = [[1]]


def length(min_count: int, max_count: int):
    return [[2, min_count, max_count]]


def uint_range(min_value: int, max_value: int):
    return [[4, min_value, max_value]]


def canonical_set(path, min_count: int, max_count: int):
    return [[3, path, min_count, max_count]]


def field(position: int, name: str, type_expr, constraints=NO_EXTRA):
    return [position, name, type_expr, constraints]


def descriptor(name: str, fields: list[list], cross_constraints=None):
    return [name, 1, fields, cross_constraints or []]


CATALOG_ROWS = [
    [1, "Observation"],
    [2, "Effect"],
    [3, "RepositoryCapacity"],
    [4, "InstallationCapacity"],
    [5, "Ceremony"],
    [6, "ActionLeaf"],
    [7, "RepositoryContinuity"],
    [8, "InstallationContinuity"],
    [9, "ActionSpec"],
]

PROFILE_KIND_ROWS = [[1, "Owner"], [2, "Protocol"], [3, "Policy"]]

OWNER_ROWS = [
    [1, "Work"],
    [2, "Contract"],
    [3, "Design"],
    [4, "Decision"],
    [5, "Execution"],
    [6, "Evidence"],
    [7, "GatePolicy"],
    [8, "Authority"],
    [9, "Coordination"],
    [10, "Orchestration"],
    [11, "Planning"],
    [12, "Projection"],
    [13, "Persistence"],
    [14, "SearchMaintenance"],
    [15, "Memory"],
    [16, "Intake"],
    [17, "Research"],
    [18, "Integration"],
    [19, "Distribution"],
    [20, "Installation"],
]

OWNER_SCOPE_ROWS = [
    [1, "GrammarSymbolAllowList"],
    [2, "NoGrammarSymbolMembership"],
]

POLICY_KIND_ROWS = [
    [1, "Migration"],
    [2, "AdapterParity"],
    [3, "Removal"],
    [4, "Proof"],
    [5, "FiniteBounds"],
    [6, "Reopen"],
]

EQUALITY_ROWS = [
    [1, "NotApplicable"],
    [2, "ExactIdentity"],
    [3, "ExactSet"],
    [4, "ExactOrderedSet"],
    [5, "OneToOne"],
    [6, "ManyToOne"],
    [7, "OneToMany"],
    [8, "AcyclicDependency"],
    [9, "Disjoint"],
    [10, "TotalFunction"],
]

DAG_ROLE_ROWS = [
    [1, "Bootstrap"],
    [2, "SourceCatalog"],
    [3, "DependentCatalog"],
    [4, "TerminalCatalog"],
]

REF_ROLE_ROWS = [
    [1, "GeneratedSum"],
    [2, "Descriptor"],
    [3, "ManifestHeader"],
    [4, "ManifestValue"],
    [5, "Subject"],
    [6, "Payload"],
    [7, "Uniqueness"],
    [8, "Attachment"],
    [9, "Projection"],
    [10, "Proof"],
]


schemas: dict[str, dict] = {}


def add_schema(name: str, value):
    envelope = ["maestro.vnext.schema.v1", value]
    schema_id, cbor_hex, byte_length = digest(envelope)
    schemas[name] = {
        "value": value,
        "schema_id": schema_id,
        "identity_envelope": envelope,
        "cbor_hex": cbor_hex,
        "byte_length": byte_length,
        "sha256": schema_id,
    }


def schema_ref(name: str):
    row = schemas[name]
    return [5, name, 1, b32(row["schema_id"])]


add_schema(
    "maestro.vnext.catalog.normative-clause.v1",
    descriptor(
        "maestro.vnext.catalog.normative-clause.v1",
        [
            field(1, "clause_tag", U64, uint_range(1, 65535)),
            field(2, "clause_name", ASCII, length(1, 128)),
            field(3, "full_clause_text", ASCII, length(1, 4096)),
        ],
    ),
)

add_schema(
    "maestro.vnext.catalog.exact-schema-ref.v1",
    descriptor(
        "maestro.vnext.catalog.exact-schema-ref.v1",
        [
            field(1, "ref_tag", U64, uint_range(1, 65535)),
            field(2, "schema_name", ASCII, length(1, 192)),
            field(3, "schema_version", U64, uint_range(1, 65535)),
            field(4, "schema_id", BYTES32),
        ],
    ),
)

add_schema(
    "maestro.vnext.catalog.tagged-schema-ref.v1",
    descriptor(
        "maestro.vnext.catalog.tagged-schema-ref.v1",
        [
            field(1, "ref_tag", U64, uint_range(1, 65535)),
            field(2, "role_tag", enum("CatalogSchemaRoleV1", REF_ROLE_ROWS)),
            field(3, "schema_ref", schema_ref("maestro.vnext.catalog.exact-schema-ref.v1")),
        ],
    ),
)

add_schema(
    "maestro.vnext.catalog.manifest-dependency-ref.v1",
    descriptor(
        "maestro.vnext.catalog.manifest-dependency-ref.v1",
        [
            field(1, "dependency_tag", U64, uint_range(1, 9)),
            field(2, "catalog_tag", enum("CatalogTagV1", CATALOG_ROWS)),
            field(3, "manifest_id", BYTES32),
        ],
    ),
)

add_schema(
    "maestro.vnext.catalog.owner-profile-ref.v1",
    descriptor(
        "maestro.vnext.catalog.owner-profile-ref.v1",
        [
            field(1, "owner_tag", U64, uint_range(1, len(OWNER_ROWS))),
            field(2, "owner_profile_id", BYTES32),
        ],
    ),
)

add_schema(
    "maestro.vnext.catalog.protocol-profile-ref.v1",
    descriptor(
        "maestro.vnext.catalog.protocol-profile-ref.v1",
        [
            field(1, "protocol_tag", U64, uint_range(1, 65535)),
            field(2, "protocol_profile_id", BYTES32),
        ],
    ),
)

add_schema(
    "maestro.vnext.catalog.policy-profile-ref.v1",
    descriptor(
        "maestro.vnext.catalog.policy-profile-ref.v1",
        [
            field(1, "policy_tag", U64, uint_range(1, 65535)),
            field(2, "policy_kind", enum("PolicyKindTagV1", POLICY_KIND_ROWS)),
            field(3, "policy_profile_id", BYTES32),
        ],
    ),
)

add_schema(
    "maestro.vnext.catalog.member-ref.v1",
    descriptor(
        "maestro.vnext.catalog.member-ref.v1",
        [
            field(1, "member_key", U64, uint_range(1, 999999999)),
            field(2, "catalog_tag", enum("CatalogTagV1", CATALOG_ROWS)),
            field(3, "member_tag", U64, uint_range(1, 65535)),
            field(4, "member_name", ASCII, length(1, 192)),
            field(5, "member_version", U64, uint_range(1, 65535)),
        ],
    ),
)

add_schema(
    "maestro.vnext.catalog.action-leaf-symbol.v1",
    descriptor(
        "maestro.vnext.catalog.action-leaf-symbol.v1",
        [
            field(1, "global_leaf_tag", U64, uint_range(1, 65535)),
            field(2, "owner_ref", schema_ref("maestro.vnext.catalog.owner-profile-ref.v1")),
            field(3, "family_tag", U64, uint_range(1, 65535)),
            field(4, "leaf_tag", U64, uint_range(1, 65535)),
            field(5, "leaf_name", ASCII, length(1, 192)),
            field(6, "leaf_schema_version", U64, uint_range(1, 65535)),
        ],
    ),
)

add_schema(
    "maestro.vnext.catalog.ceremony-symbol.v1",
    descriptor(
        "maestro.vnext.catalog.ceremony-symbol.v1",
        [
            field(1, "ceremony_tag", U64, uint_range(1, 65535)),
            field(2, "owner_ref", schema_ref("maestro.vnext.catalog.owner-profile-ref.v1")),
            field(3, "ceremony_name", ASCII, length(1, 192)),
            field(4, "ceremony_schema_version", U64, uint_range(1, 65535)),
        ],
    ),
)

ROUTE_ROLE_ROWS = [
    [1, "ActionReserve"],
    [2, "ActionOutcome"],
    [3, "ActionReconcile"],
    [4, "CeremonyInitiate"],
    [5, "CeremonyResolveResult"],
]

ROUTE_BASIS_ROWS = [
    [1, "Ordinary"],
    [2, "BootstrapG0"],
    [3, "ContinuityMaintenance"],
    [4, "CeremonyExternal"],
]

ROUTE_CONTEXT_ROWS = [
    [1, "ActiveStore"],
    [2, "NoStore"],
    [3, "PreStore"],
]

SYMBOL_KIND_ROWS = [[1, "ActionLeaf"], [2, "Ceremony"]]
ORIGIN_SOURCE_OWNER_ROWS = [
    [1, "Execution"],
    [2, "Coordination"],
    [3, "Authority"],
    [4, "Installation"],
    [5, "Distribution"],
    [6, "Evidence"],
]

add_schema(
    "maestro.vnext.catalog.effect-origin-route-entry.v1",
    descriptor(
        "maestro.vnext.catalog.effect-origin-route-entry.v1",
        [
            field(1, "route_tag", U64, uint_range(1, 65535)),
            field(2, "route_role", enum("EffectOriginRouteRoleV1", ROUTE_ROLE_ROWS)),
            field(3, "route_context", enum("EffectOriginRouteContextV1", ROUTE_CONTEXT_ROWS)),
            field(4, "route_basis", enum("EffectOriginRouteBasisV1", ROUTE_BASIS_ROWS)),
            field(5, "symbol_kind", enum("EffectOriginRouteSymbolKindV1", SYMBOL_KIND_ROWS)),
            field(6, "symbol_tag", U64, uint_range(1, 65535)),
            field(7, "symbol_id", BYTES32),
        ],
    ),
)

add_schema(
    "maestro.vnext.catalog.effect-origin-route.v1",
    descriptor(
        "maestro.vnext.catalog.effect-origin-route.v1",
        [
            field(1, "origin_tag", U64, uint_range(1, 23)),
            field(2, "origin_name", ASCII, length(1, 192)),
            field(3, "origin_source_owner", enum("EffectOriginSourceOwnerV1", ORIGIN_SOURCE_OWNER_ROWS)),
            field(
                4,
                "canonical_route_set",
                ordered_list(schema_ref("maestro.vnext.catalog.effect-origin-route-entry.v1")),
                canonical_set([[1, 1]], 2, 8),
            ),
        ],
    ),
)

add_schema(
    "maestro.vnext.catalog.owner-profile.v1",
    descriptor(
        "maestro.vnext.catalog.owner-profile.v1",
        [
            field(1, "owner_tag", enum("CatalogOwnerTagV1", OWNER_ROWS)),
            field(2, "owner_name", ASCII, length(1, 64)),
            field(3, "profile_version", U64, uint_range(1, 65535)),
            field(4, "owner_dictionary_id", BYTES32),
            field(
                5,
                "grammar_member_catalog_tags",
                ordered_list(tuple_of(U64)),
                canonical_set([[2, 0]], 2, 2),
            ),
            field(6, "grammar_membership_mode", enum("CatalogOwnerScopeModeV1", OWNER_SCOPE_ROWS)),
            field(
                7,
                "allowed_grammar_members",
                ordered_list(schema_ref("maestro.vnext.catalog.member-ref.v1")),
                canonical_set([[1, 1]], 0, 512),
            ),
            field(8, "owns_action_family_symbol", BOOL),
            field(
                9,
                "boundary_clauses",
                ordered_list(schema_ref("maestro.vnext.catalog.normative-clause.v1")),
                canonical_set([[1, 1]], 1, 64),
            ),
        ],
    ),
)

add_schema(
    "maestro.vnext.catalog.protocol-profile.v1",
    descriptor(
        "maestro.vnext.catalog.protocol-profile.v1",
        [
            field(1, "protocol_tag", U64, uint_range(1, 65535)),
            field(2, "protocol_name", ASCII, length(1, 128)),
            field(3, "protocol_version", U64, uint_range(1, 65535)),
            field(4, "profile_dictionary_id", BYTES32),
            field(5, "owner_ref", schema_ref("maestro.vnext.catalog.owner-profile-ref.v1")),
            field(
                6,
                "participant_owner_refs",
                ordered_list(schema_ref("maestro.vnext.catalog.owner-profile-ref.v1")),
                canonical_set([[1, 1]], 0, 20),
            ),
            field(
                7,
                "schema_refs",
                ordered_list(schema_ref("maestro.vnext.catalog.tagged-schema-ref.v1")),
                canonical_set([[1, 1]], 0, 512),
            ),
            field(
                8,
                "dependency_protocol_refs",
                ordered_list(schema_ref("maestro.vnext.catalog.protocol-profile-ref.v1")),
                canonical_set([[1, 1]], 0, 128),
            ),
            field(9, "minimum_cardinality", U64, uint_range(0, 65535)),
            field(10, "maximum_cardinality", U64, uint_range(0, 65535)),
            field(11, "equality_mode", enum("CatalogEqualityModeV1", EQUALITY_ROWS)),
            field(12, "dag_role", enum("CatalogDagRoleV1", DAG_ROLE_ROWS)),
            field(
                13,
                "boundary_clauses",
                ordered_list(schema_ref("maestro.vnext.catalog.normative-clause.v1")),
                canonical_set([[1, 1]], 1, 128),
            ),
        ],
    ),
)

add_schema(
    "maestro.vnext.catalog.policy-profile.v1",
    descriptor(
        "maestro.vnext.catalog.policy-profile.v1",
        [
            field(1, "policy_tag", U64, uint_range(1, 65535)),
            field(2, "policy_name", ASCII, length(1, 128)),
            field(3, "policy_kind", enum("PolicyKindTagV1", POLICY_KIND_ROWS)),
            field(4, "policy_version", U64, uint_range(1, 65535)),
            field(5, "profile_dictionary_id", BYTES32),
            field(6, "owner_ref", schema_ref("maestro.vnext.catalog.owner-profile-ref.v1")),
            field(
                7,
                "schema_refs",
                ordered_list(schema_ref("maestro.vnext.catalog.tagged-schema-ref.v1")),
                canonical_set([[1, 1]], 0, 512),
            ),
            field(8, "minimum_cardinality", U64, uint_range(0, 65535)),
            field(9, "maximum_cardinality", U64, uint_range(0, 65535)),
            field(10, "equality_mode", enum("CatalogEqualityModeV1", EQUALITY_ROWS)),
            field(
                11,
                "boundary_clauses",
                ordered_list(schema_ref("maestro.vnext.catalog.normative-clause.v1")),
                canonical_set([[1, 1]], 1, 128),
            ),
        ],
    ),
)

for set_name, row_name, row_count in [
    ("owner-profile-set", "maestro.vnext.catalog.owner-profile.v1", 20),
    ("protocol-profile-set", "maestro.vnext.catalog.protocol-profile.v1", 6),
    ("policy-profile-set", "maestro.vnext.catalog.policy-profile.v1", 6),
]:
    row_schema_name = f"maestro.vnext.catalog.{set_name}-row.v1"
    add_schema(
        row_schema_name,
        descriptor(
            row_schema_name,
            [
                field(1, "profile_tag", U64, uint_range(1, row_count)),
                field(2, "profile_id", BYTES32),
                field(3, "profile_value", schema_ref(row_name)),
            ],
        ),
    )
    schema_name = f"maestro.vnext.catalog.{set_name}.v1"
    add_schema(
        schema_name,
        descriptor(
            schema_name,
            [
                field(1, "set_version", U64, uint_range(1, 1)),
                field(2, "row_count", U64, uint_range(row_count, row_count)),
                field(3, "min_tag", U64, uint_range(1, 1)),
                field(4, "max_tag", U64, uint_range(row_count, row_count)),
                field(
                    5,
                    "rows",
                    ordered_list(schema_ref(row_schema_name)),
                    canonical_set([[1, 1]], row_count, row_count),
                ),
            ],
        ),
    )

add_schema(
    "maestro.vnext.catalog.profile-set-ref.v1",
    descriptor(
        "maestro.vnext.catalog.profile-set-ref.v1",
        [
            field(1, "ref_tag", U64, uint_range(1, 3)),
            field(2, "profile_kind", enum("ProfileKindTagV1", PROFILE_KIND_ROWS)),
            field(3, "profile_set_id", BYTES32),
        ],
        [[1, [[1, 1]], [[1, 2]]]],
    ),
)

add_schema(
    "maestro.vnext.catalog.dependency-edge.v1",
    descriptor(
        "maestro.vnext.catalog.dependency-edge.v1",
        [
            field(1, "edge_tag", U64, uint_range(1, 28)),
            field(2, "dependent_catalog", enum("CatalogTagV1", CATALOG_ROWS)),
            field(3, "predecessor_catalog", enum("CatalogTagV1", CATALOG_ROWS)),
        ],
    ),
)

add_schema(
    "maestro.vnext.catalog.manifest-header-shape.v1",
    descriptor(
        "maestro.vnext.catalog.manifest-header-shape.v1",
        [
            field(1, "catalog_tag", enum("CatalogTagV1", CATALOG_ROWS)),
            field(2, "catalog_version", U64, uint_range(1, 65535)),
            field(3, "core_version", U64, uint_range(1, 65535)),
            field(4, "canonicalization_version", U64, uint_range(1, 65535)),
            field(5, "catalog_profile_grammar_id", BYTES32),
            field(6, "generated_sum_schema_id", BYTES32),
            field(7, "descriptor_schema_id", BYTES32),
            field(
                8,
                "manifest_dependencies",
                ordered_list(schema_ref("maestro.vnext.catalog.manifest-dependency-ref.v1")),
                canonical_set([[1, 1]], 0, 8),
            ),
            field(
                9,
                "owner_profile_refs",
                ordered_list(schema_ref("maestro.vnext.catalog.owner-profile-ref.v1")),
                canonical_set([[1, 1]], 1, 20),
            ),
            field(
                10,
                "protocol_profile_refs",
                ordered_list(schema_ref("maestro.vnext.catalog.protocol-profile-ref.v1")),
                canonical_set([[1, 1]], 1, 128),
            ),
            field(11, "migration_policy_ref", schema_ref("maestro.vnext.catalog.policy-profile-ref.v1")),
            field(12, "parity_policy_ref", schema_ref("maestro.vnext.catalog.policy-profile-ref.v1")),
            field(13, "removal_policy_ref", schema_ref("maestro.vnext.catalog.policy-profile-ref.v1")),
            field(14, "proof_policy_ref", schema_ref("maestro.vnext.catalog.policy-profile-ref.v1")),
            field(15, "owner_dictionary_id", BYTES32),
            field(16, "profile_dictionary_id", BYTES32),
            field(17, "row_count", U64, uint_range(1, 65535)),
            field(18, "min_tag", U64, uint_range(1, 1)),
            field(19, "max_tag", U64, uint_range(1, 65535)),
        ],
    ),
)


DICTIONARY_DEFS = [
    (1, "CatalogTagV1", CATALOG_ROWS),
    (2, "ProfileKindTagV1", PROFILE_KIND_ROWS),
    (3, "CatalogOwnerTagV1", OWNER_ROWS),
    (4, "CatalogOwnerScopeModeV1", OWNER_SCOPE_ROWS),
    (5, "PolicyKindTagV1", POLICY_KIND_ROWS),
    (6, "CatalogEqualityModeV1", EQUALITY_ROWS),
    (7, "CatalogDagRoleV1", DAG_ROLE_ROWS),
    (8, "CatalogSchemaRoleV1", REF_ROLE_ROWS),
    (9, "EffectOriginRouteRoleV1", ROUTE_ROLE_ROWS),
    (10, "EffectOriginRouteBasisV1", ROUTE_BASIS_ROWS),
    (11, "EffectOriginRouteContextV1", ROUTE_CONTEXT_ROWS),
    (12, "EffectOriginRouteSymbolKindV1", SYMBOL_KIND_ROWS),
    (13, "EffectOriginSourceOwnerV1", ORIGIN_SOURCE_OWNER_ROWS),
]

add_schema(
    "maestro.vnext.catalog.dictionary-row.v1",
    descriptor(
        "maestro.vnext.catalog.dictionary-row.v1",
        [
            field(1, "row_tag", U64, uint_range(1, 65535)),
            field(2, "row_name", ASCII, length(1, 192)),
        ],
    ),
)


def dictionary_schema_name(dictionary_name: str) -> str:
    normalized = []
    for character in dictionary_name.removesuffix("V1"):
        if character.isupper() and normalized:
            normalized.append("-")
        normalized.append(character.lower())
    return "maestro.vnext.catalog.dictionary." + "".join(normalized) + ".v1"


for dictionary_tag, dictionary_name, rows in DICTIONARY_DEFS:
    row_count = len(rows)
    schema_name = dictionary_schema_name(dictionary_name)
    add_schema(
        schema_name,
        descriptor(
            schema_name,
            [
                field(1, "dictionary_tag", U64, uint_range(dictionary_tag, dictionary_tag)),
                field(2, "dictionary_version", U64, uint_range(1, 1)),
                field(3, "row_count", U64, uint_range(row_count, row_count)),
                field(4, "min_tag", U64, uint_range(1, 1)),
                field(5, "max_tag", U64, uint_range(row_count, row_count)),
                field(
                    6,
                    "rows",
                    ordered_list(schema_ref("maestro.vnext.catalog.dictionary-row.v1")),
                    canonical_set([[1, 1]], row_count, row_count),
                ),
            ],
        ),
    )

add_schema(
    "maestro.vnext.catalog.owner-dictionary.v1",
    descriptor(
        "maestro.vnext.catalog.owner-dictionary.v1",
        [
            field(1, "dictionary_version", U64, uint_range(1, 1)),
            field(2, "owner_tag_dictionary_id", BYTES32),
            field(3, "owner_scope_dictionary_id", BYTES32),
        ],
    ),
)

add_schema(
    "maestro.vnext.catalog.profile-dictionary.v1",
    descriptor(
        "maestro.vnext.catalog.profile-dictionary.v1",
        [
            field(1, "dictionary_version", U64, uint_range(1, 1)),
            field(2, "profile_kind_dictionary_id", BYTES32),
            field(3, "policy_kind_dictionary_id", BYTES32),
            field(4, "equality_mode_dictionary_id", BYTES32),
            field(5, "dag_role_dictionary_id", BYTES32),
            field(6, "schema_role_dictionary_id", BYTES32),
            field(7, "route_role_dictionary_id", BYTES32),
            field(8, "route_basis_dictionary_id", BYTES32),
            field(9, "route_context_dictionary_id", BYTES32),
            field(10, "route_symbol_kind_dictionary_id", BYTES32),
            field(11, "origin_source_owner_dictionary_id", BYTES32),
        ],
    ),
)

# This schema is deliberately added last: its exact schema-ref cardinality includes
# itself without creating a value or identity self-hash.
grammar_schema_count = len(schemas) + 1
add_schema(
    "maestro.vnext.catalog.profile-grammar-value.v1",
    descriptor(
        "maestro.vnext.catalog.profile-grammar-value.v1",
        [
            field(1, "grammar_version", U64, uint_range(1, 1)),
            field(2, "catalog_tag_dictionary_id", BYTES32),
            field(3, "owner_dictionary_id", BYTES32),
            field(4, "profile_dictionary_id", BYTES32),
            field(
                5,
                "schema_refs",
                ordered_list(schema_ref("maestro.vnext.catalog.exact-schema-ref.v1")),
                canonical_set([[1, 1]], grammar_schema_count, grammar_schema_count),
            ),
            field(
                6,
                "profile_set_refs",
                ordered_list(schema_ref("maestro.vnext.catalog.profile-set-ref.v1")),
                canonical_set([[1, 1]], 3, 3),
            ),
            field(
                7,
                "dependency_dag_edges",
                ordered_list(schema_ref("maestro.vnext.catalog.dependency-edge.v1")),
                canonical_set([[1, 1]], 28, 28),
            ),
            field(8, "dictionary_count", U64, uint_range(13, 13)),
            field(9, "schema_count", U64, uint_range(grammar_schema_count, grammar_schema_count)),
            field(10, "owner_profile_count", U64, uint_range(20, 20)),
            field(11, "protocol_profile_count", U64, uint_range(6, 6)),
            field(12, "policy_profile_count", U64, uint_range(6, 6)),
            field(
                13,
                "action_leaf_symbols",
                ordered_list(schema_ref("maestro.vnext.catalog.action-leaf-symbol.v1")),
                canonical_set([[1, 1]], 136, 136),
            ),
            field(
                14,
                "ceremony_symbols",
                ordered_list(schema_ref("maestro.vnext.catalog.ceremony-symbol.v1")),
                canonical_set([[1, 1]], 11, 11),
            ),
            field(
                15,
                "effect_origin_routes",
                ordered_list(schema_ref("maestro.vnext.catalog.effect-origin-route.v1")),
                canonical_set([[1, 1]], 23, 23),
            ),
            field(
                16,
                "boundary_clauses",
                ordered_list(schema_ref("maestro.vnext.catalog.normative-clause.v1")),
                canonical_set([[1, 1]], 5, 5),
            ),
        ],
    ),
)


def exact_schema_ref_value(tag: int, name: str):
    row = schemas[name]
    return [tag, name, 1, b32(row["schema_id"])]


def tagged_schema_ref_value(tag: int, role: int, name: str):
    return [tag, role, exact_schema_ref_value(tag, name)]


def clause(tag: int, name: str, text: str):
    text.encode("ascii")
    return [tag, name, text]


def dictionary_value(dictionary_tag: int, rows: list[list]):
    return [dictionary_tag, 1, len(rows), 1, len(rows), rows]


def dictionary_artifact(dictionary_tag: int, name: str, rows: list[list]):
    value = dictionary_value(dictionary_tag, rows)
    schema_name = dictionary_schema_name(name)
    envelope = [
        "maestro.vnext.catalog.dictionary.v1",
        b32(schemas[schema_name]["schema_id"]),
        value,
    ]
    identifier, cbor_hex, byte_length = digest(envelope)
    return {
        "schema_name": schema_name,
        "schema_id": schemas[schema_name]["schema_id"],
        "value": value,
        "dictionary_id": identifier,
        "identity_envelope": envelope,
        "cbor_hex": cbor_hex,
        "byte_length": byte_length,
        "sha256": identifier,
    }


dictionaries = {
    name: dictionary_artifact(dictionary_tag, name, rows)
    for dictionary_tag, name, rows in DICTIONARY_DEFS
}

owner_dictionary_value = [
    1,
    b32(dictionaries["CatalogOwnerTagV1"]["dictionary_id"]),
    b32(dictionaries["CatalogOwnerScopeModeV1"]["dictionary_id"]),
]
owner_dictionary_id, owner_dictionary_hex, owner_dictionary_len = digest(
    [
        "maestro.vnext.catalog.owner-dictionary.v1",
        b32(schemas["maestro.vnext.catalog.owner-dictionary.v1"]["schema_id"]),
        owner_dictionary_value,
    ]
)

profile_dictionary_value = [
    1,
    b32(dictionaries["ProfileKindTagV1"]["dictionary_id"]),
    b32(dictionaries["PolicyKindTagV1"]["dictionary_id"]),
    b32(dictionaries["CatalogEqualityModeV1"]["dictionary_id"]),
    b32(dictionaries["CatalogDagRoleV1"]["dictionary_id"]),
    b32(dictionaries["CatalogSchemaRoleV1"]["dictionary_id"]),
    b32(dictionaries["EffectOriginRouteRoleV1"]["dictionary_id"]),
    b32(dictionaries["EffectOriginRouteBasisV1"]["dictionary_id"]),
    b32(dictionaries["EffectOriginRouteContextV1"]["dictionary_id"]),
    b32(dictionaries["EffectOriginRouteSymbolKindV1"]["dictionary_id"]),
    b32(dictionaries["EffectOriginSourceOwnerV1"]["dictionary_id"]),
]
profile_dictionary_id, profile_dictionary_hex, profile_dictionary_len = digest(
    [
        "maestro.vnext.catalog.profile-dictionary.v1",
        b32(schemas["maestro.vnext.catalog.profile-dictionary.v1"]["schema_id"]),
        profile_dictionary_value,
    ]
)


OWNER_TAG_BY_NAME = {name: tag for tag, name in OWNER_ROWS}
ACTION_OWNER_TAGS = {
    OWNER_TAG_BY_NAME[row["owner"]] for row in SOURCE["action_families"]
}
ALLOWED_MEMBER_REFS = {tag: [] for tag, _ in OWNER_ROWS}

global_action_tag = 0
for family in SOURCE["action_families"]:
    owner_tag = OWNER_TAG_BY_NAME[family["owner"]]
    for leaf_name in family["leaves"]:
        global_action_tag += 1
        ALLOWED_MEMBER_REFS[owner_tag].append(
            [6_000_000 + global_action_tag, 6, global_action_tag, leaf_name, 1]
        )

for ceremony_tag, ceremony in enumerate(SOURCE["ceremonies"], 1):
    owner_tag = OWNER_TAG_BY_NAME[ceremony["owner"]]
    ALLOWED_MEMBER_REFS[owner_tag].append(
        [5_000_000 + ceremony_tag, 5, ceremony_tag, ceremony["name"], 1]
    )


def owner_profile_value(tag: int, name: str):
    allowed = ALLOWED_MEMBER_REFS[tag]
    scope_mode = 1 if allowed else 2
    boundary = (
        f"{name} may own only the exact ActionLeaf and Ceremony member references listed "
        "in this profile; membership grants no runtime authority, applicability, currentness, "
        "Lease, Attempt, Run, retry, Receipt, Result or mutation right."
    )
    return [
        tag,
        name,
        1,
        b32(owner_dictionary_id),
        [[5], [6]],
        scope_mode,
        allowed,
        tag in ACTION_OWNER_TAGS,
        [clause(1, "owner_boundary", boundary)],
    ]


owner_values = [owner_profile_value(tag, name) for tag, name in OWNER_ROWS]


def profile_id(domain: str, schema_name: str, value):
    envelope = [domain, b32(schemas[schema_name]["schema_id"]), value]
    identifier, cbor_hex, byte_length = digest(envelope)
    return identifier, envelope, cbor_hex, byte_length


owner_rows = []
owner_by_tag = {}
owner_artifacts = []
for value in owner_values:
    tag = value[0]
    identifier, envelope, cbor_hex, byte_length = profile_id(
        "maestro.vnext.catalog.owner-profile.v1",
        "maestro.vnext.catalog.owner-profile.v1",
        value,
    )
    owner_rows.append([tag, b32(identifier), value])
    owner_by_tag[tag] = identifier
    owner_artifacts.append(
        {
            "tag": tag,
            "name": value[1],
            "value": value,
            "profile_id": identifier,
            "identity_envelope": envelope,
            "cbor_hex": cbor_hex,
            "byte_length": byte_length,
            "sha256": identifier,
        }
    )


def owner_ref_value(tag: int):
    return [tag, b32(owner_by_tag[tag])]


common_schema_names = list(schemas)


protocol_values = [
    [
        1,
        "CatalogProfileGrammarV1",
        1,
        owner_ref_value(2),
        [owner_ref_value(3), owner_ref_value(6), owner_ref_value(13), owner_ref_value(18)],
        [tagged_schema_ref_value(i + 1, 2, name) for i, name in enumerate(common_schema_names)],
        [],
        1,
        65535,
        3,
        1,
        [
            clause(1, "compile_time_only", "Profiles and descriptors compile into closed core types and may never be interpreted as runtime behavior, authority, currentness, or extension registration."),
            clause(2, "step_absence", "CatalogOwnerTagV1 is limited to primary owners appearing in the nine catalogs; Step remains an architectural context and typed subject but owns no primary catalog row in V1."),
        ],
    ],
    [
        2,
        "CatalogOwnerClosureV1",
        1,
        owner_ref_value(2),
        [owner_ref_value(3), owner_ref_value(8)],
        [tagged_schema_ref_value(1, 2, "maestro.vnext.catalog.owner-profile.v1")],
        [],
        20,
        20,
        3,
        1,
        [
              clause(1, "split_plane_owner_closure", "OwnerProfileV1 partitions only the exact ActionLeaf and Ceremony grammar symbols. Every later catalog binds its own complete PrimaryOwnerRelationV1; global owner closure is the conjunction of the nine manifest-local total functions."),
            clause(2, "closed_owner_dictionary", "CatalogOwnerTagV1 contains exactly twenty dense tags; unknown, duplicate, aliased, or adapter-supplied owners fail closed."),
        ],
    ],
    [
        3,
        "CatalogDependencyDagV1",
        1,
        owner_ref_value(2),
        [owner_ref_value(13)],
        [tagged_schema_ref_value(1, 8, "maestro.vnext.catalog.manifest-dependency-ref.v1")],
          [],
          28,
          28,
        8,
        1,
        [
            clause(1, "strict_backward_edges", "Every Manifest dependency edge points to an already frozen predecessor; forward edges, self hashes, latest selectors, and implicit dependencies fail closed."),
        ],
    ],
    [
        4,
        "CatalogDenseTagClosureV1",
        1,
        owner_ref_value(2),
        [owner_ref_value(13)],
        [tagged_schema_ref_value(1, 3, "maestro.vnext.catalog.manifest-header-shape.v1")],
        [],
        1,
        65535,
        4,
        2,
        [
            clause(1, "dense_tags", "Each nominal catalog schema fixes min_tag as one, list length as row_count, row_count as max_tag, strict unique tag order, and every tag in the closed range one through max_tag."),
            clause(2, "nominal_generation", "Each of the nine catalog Decisions publishes its own exact HeaderV1, row, descriptor, generated sum, and ManifestValueV1 schemas with literal finite bounds."),
        ],
    ],
      [
          5,
            "EffectOriginRouteClosureV1",
          1,
          owner_ref_value(2),
          [owner_ref_value(5), owner_ref_value(8)],
          [
              tagged_schema_ref_value(1, 2, "maestro.vnext.catalog.action-leaf-symbol.v1"),
              tagged_schema_ref_value(2, 2, "maestro.vnext.catalog.ceremony-symbol.v1"),
                tagged_schema_ref_value(3, 2, "maestro.vnext.catalog.effect-origin-route-entry.v1"),
                tagged_schema_ref_value(4, 2, "maestro.vnext.catalog.effect-origin-route.v1"),
          ],
          [],
            79,
            79,
          3,
        2,
        [
              clause(1, "pre_manifest_symbol", "An ActionLeafSymbolV1 is a non-executable content commitment that grants no Action, authority, applicability, Packet advertisement, or mutation right."),
              clause(2, "ceremony_symbol", "A CeremonySymbolV1 is the equally non-executable content commitment for one closed Ceremony operation and grants no authority, support, applicability, currentness, or submission right."),
              clause(3, "route_closure", "Exactly twenty-three origins bind exactly seventy-nine route entries. Every route content-binds role, context, basis and an exact ActionLeafSymbolV1 or CeremonySymbolV1 identity; empty, duplicate, phase-incomplete, cross-context or cross-basis routes fail closed."),
          ],
      ],
    [
        6,
        "GeneratedClosedSumEncodingV1",
        1,
        owner_ref_value(2),
        [owner_ref_value(13)],
        [],
        [],
        1,
        65535,
        4,
        2,
        [
              clause(1, "optional_field_sum", "A generated closed sum is one nominal SchemaDescriptor with one optional typed variant field per dense V1 tag and one exactly-one-present cross constraint; field position is the variant tag."),
              clause(2, "exact_record_arity", "Every SchemaDescriptor value is one definite array with exact arity and one encoded value per field in schema order; optional fields encode only as absent [0] or present [1,value]."),
              clause(3, "no_tag_payload_mismatch", "Generated closed sums carry no separately supplied discriminant, open payload, callback, map, string kind, unknown variant, or runtime registry; an independent equality proof binds every manifest row tag, name, and payload SchemaId to its generated field."),
        ],
    ],
]

for value in protocol_values:
    value.insert(3, b32(profile_dictionary_id))


protocol_rows = []
protocol_by_tag = {}
protocol_artifacts = []
for value in protocol_values:
    tag = value[0]
    identifier, envelope, cbor_hex, byte_length = profile_id(
        "maestro.vnext.catalog.protocol-profile.v1",
        "maestro.vnext.catalog.protocol-profile.v1",
        value,
    )
    protocol_rows.append([tag, b32(identifier), value])
    protocol_by_tag[tag] = identifier
    protocol_artifacts.append(
        {
            "tag": tag,
            "name": value[1],
            "value": value,
            "profile_id": identifier,
            "identity_envelope": envelope,
            "cbor_hex": cbor_hex,
            "byte_length": byte_length,
            "sha256": identifier,
        }
    )


policy_values = [
    [
        1,
        "ByteTotalNonPromotingMigrationV1",
        1,
        1,
        owner_ref_value(13),
        [],
        0,
        65535,
        3,
        [
            clause(1, "byte_total", "Migration preserves every source byte and provenance or records explicit preexisting loss; it never infers a profile, tag, schema, class, authority, currentness, replay right, or executable unknown."),
        ],
    ],
    [
        2,
        "ExactAdapterParityV1",
        2,
        1,
        owner_ref_value(18),
        [],
        0,
        65535,
        3,
        [
            clause(1, "same_typed_value", "CLI, JSON, MCP, TUI, hooks, shell, skills, Recipes, and connectors round trip the same typed value and bounded refusal without supplying or privately interpreting profile semantics."),
        ],
    ],
    [
        3,
        "ConsumerTotalRemovalV1",
        3,
        1,
        owner_ref_value(2),
        [],
        0,
        65535,
        3,
        [
            clause(1, "consumer_total", "Removal requires an exact generated zero-live-consumer proof across runtime, migration, rollback, sealed export, retained binary, schema, adapter, resource, documentation, and test dependencies."),
        ],
    ],
    [
        4,
        "IndependentCatalogProofV1",
        4,
        1,
        owner_ref_value(6),
        [],
        0,
        65535,
        3,
        [
            clause(1, "mutant_totality", "Proof rejects omission, extra, duplicate, gap, wrong owner, wrong key path, forward edge, self hash, swapped profile, changed clause, unknown version, stale dependency, and adapter divergence mutants."),
            clause(2, "encoder_equality", "Every frozen catalog includes two independent reference encoder receipts over all literal identity envelopes; any mismatch is an integrity failure."),
        ],
    ],
    [
        5,
        "CommonFiniteBoundsV1",
        5,
        1,
        owner_ref_value(2),
        [tagged_schema_ref_value(1, 3, "maestro.vnext.catalog.manifest-header-shape.v1")],
        1,
        65535,
        2,
        [
            clause(1, "finite_catalog", "V1 catalog tags, rows, references, clauses, attachments, subjects, payloads, proof inputs, and generated variants all have literal nonzero finite maxima in their nominal schemas."),
        ],
    ],
    [
        6,
        "FailClosedCatalogReopenV1",
        6,
        1,
        owner_ref_value(3),
        [],
        0,
        65535,
        3,
        [
            clause(1, "reopen", "Reopen when a legitimate closed semantic value cannot fit the frozen grammar, a required owner is ambiguous, finite bounds cannot exist, a dependency cycle appears, an old client must execute unknown semantics, or independent encoders disagree."),
        ],
    ],
]

for value in policy_values:
    value.insert(4, b32(profile_dictionary_id))


policy_rows = []
policy_by_tag = {}
policy_artifacts = []
for value in policy_values:
    tag = value[0]
    identifier, envelope, cbor_hex, byte_length = profile_id(
        "maestro.vnext.catalog.policy-profile.v1",
        "maestro.vnext.catalog.policy-profile.v1",
        value,
    )
    policy_rows.append([tag, b32(identifier), value])
    policy_by_tag[tag] = identifier
    policy_artifacts.append(
        {
            "tag": tag,
            "name": value[1],
            "value": value,
            "profile_id": identifier,
            "identity_envelope": envelope,
            "cbor_hex": cbor_hex,
            "byte_length": byte_length,
            "sha256": identifier,
        }
    )


def profile_set_artifact(domain: str, schema_name: str, rows: list[list]):
    value = [1, len(rows), 1, len(rows), rows]
    envelope = [domain, b32(schemas[schema_name]["schema_id"]), value]
    identifier, cbor_hex, byte_length = digest(envelope)
    return {
        "value": value,
        "profile_set_id": identifier,
        "identity_envelope": envelope,
        "cbor_hex": cbor_hex,
        "byte_length": byte_length,
        "sha256": identifier,
    }


profile_sets = {
      "OwnerProfileSetV1": profile_set_artifact(
          "maestro.vnext.catalog.owner-profile-set.v1",
          "maestro.vnext.catalog.owner-profile-set.v1",
          owner_rows,
      ),
      "ProtocolProfileSetV1": profile_set_artifact(
          "maestro.vnext.catalog.protocol-profile-set.v1",
          "maestro.vnext.catalog.protocol-profile-set.v1",
          protocol_rows,
      ),
      "PolicyProfileSetV1": profile_set_artifact(
          "maestro.vnext.catalog.policy-profile-set.v1",
          "maestro.vnext.catalog.policy-profile-set.v1",
          policy_rows,
      ),
}


def descriptor_artifact(domain: str, schema_name: str, value):
    envelope = [domain, b32(schemas[schema_name]["schema_id"]), value]
    identifier, cbor_hex, byte_length = digest(envelope)
    return {
        "value": value,
        "descriptor_id": identifier,
        "identity_envelope": envelope,
        "cbor_hex": cbor_hex,
        "byte_length": byte_length,
        "sha256": identifier,
    }


action_symbol_artifacts = []
action_symbol_by_name = {}
global_action_tag = 0
for family_tag, family in enumerate(SOURCE["action_families"], 1):
    owner_tag = OWNER_TAG_BY_NAME[family["owner"]]
    for family_local_tag, leaf_name in enumerate(family["leaves"], 1):
        global_action_tag += 1
        value = [
            global_action_tag,
            owner_ref_value(owner_tag),
            family_tag,
            family_local_tag,
            leaf_name,
            1,
        ]
        row = descriptor_artifact(
            "maestro.vnext.catalog.action-leaf-symbol.descriptor.v1",
            "maestro.vnext.catalog.action-leaf-symbol.v1",
            value,
        )
        row.update(
            {
                "global_tag": global_action_tag,
                "family_tag": family_tag,
                "family_local_tag": family_local_tag,
                "name": leaf_name,
                "owner": family["owner"],
            }
        )
        action_symbol_artifacts.append(row)
        action_symbol_by_name[leaf_name] = row


ceremony_symbol_artifacts = []
ceremony_symbol_by_name = {}
for ceremony_tag, ceremony in enumerate(SOURCE["ceremonies"], 1):
    value = [
        ceremony_tag,
        owner_ref_value(OWNER_TAG_BY_NAME[ceremony["owner"]]),
        ceremony["name"],
        1,
    ]
    row = descriptor_artifact(
        "maestro.vnext.catalog.ceremony-symbol.descriptor.v1",
        "maestro.vnext.catalog.ceremony-symbol.v1",
        value,
    )
    row.update(
        {
            "tag": ceremony_tag,
            "name": ceremony["name"],
            "owner": ceremony["owner"],
        }
    )
    ceremony_symbol_artifacts.append(row)
    ceremony_symbol_by_name[ceremony["name"]] = row


ACTION_PHASE_SETS = {
    "OrdinaryGeneric": [
        (1, "OriginateEffectIntent"),
        (2, "RecordDispatchOutcome"),
        (3, "ReconcileEffectIntent"),
    ],
    "CoordinationDelivery": [
        (1, "OriginateCoordinationDelivery"),
        (2, "RecordDispatchOutcome"),
        (3, "ReconcileEffectIntent"),
    ],
    "BootstrapInteraction": [
        (1, "ReserveBootstrapMandateInteractionEffect"),
        (2, "PublishBootstrapMandateInteractionOutcome"),
        (3, "ReconcileBootstrapMandateInteractionEffect"),
    ],
    "ContinuityMaintenance": [
        (1, "ReserveContinuityMaintenanceEffect"),
        (2, "PublishContinuityMaintenanceEffectOutcome"),
        (3, "ReconcileContinuityMaintenanceEffect"),
    ],
}
ROUTE_CONTEXT_TAG_BY_NAME = {name: tag for tag, name in ROUTE_CONTEXT_ROWS}
ROUTE_BASIS_TAG_BY_NAME = {name: tag for tag, name in ROUTE_BASIS_ROWS}
ORIGIN_SOURCE_OWNER_TAG_BY_NAME = {name: tag for tag, name in ORIGIN_SOURCE_OWNER_ROWS}


effect_origin_route_artifacts = []
route_entry_count = 0
for origin_tag, route_source in enumerate(SOURCE["effect_origin_routes"], 1):
    assert route_source["origin"] == SOURCE["effect_origins"][origin_tag - 1]
    entries = []
    route_tag = 0
    if route_source["action_phase_set"] is not None:
        context_tag = ROUTE_CONTEXT_TAG_BY_NAME[route_source["action_context"]]
        basis_tag = ROUTE_BASIS_TAG_BY_NAME[route_source["action_basis"]]
        for role_tag, leaf_name in ACTION_PHASE_SETS[route_source["action_phase_set"]]:
            route_tag += 1
            symbol = action_symbol_by_name[leaf_name]
            entries.append(
                [route_tag, role_tag, context_tag, basis_tag, 1, symbol["global_tag"], b32(symbol["descriptor_id"])]
            )
    for ceremony_name in route_source["ceremony_symbols"]:
        symbol = ceremony_symbol_by_name[ceremony_name]
        context_tag = ROUTE_CONTEXT_TAG_BY_NAME[
            "NoStore" if symbol["tag"] == 1 else "PreStore"
        ]
        for role_tag in (4, 5):
            route_tag += 1
            entries.append(
                [route_tag, role_tag, context_tag, 4, 2, symbol["tag"], b32(symbol["descriptor_id"])]
            )
    route_entry_count += len(entries)
    value = [
        origin_tag,
        route_source["origin"],
        ORIGIN_SOURCE_OWNER_TAG_BY_NAME[route_source["origin_source_owner"]],
        entries,
    ]
    row = descriptor_artifact(
        "maestro.vnext.catalog.effect-origin-route.descriptor.v1",
        "maestro.vnext.catalog.effect-origin-route.v1",
        value,
    )
    row.update(
        {
            "origin_tag": origin_tag,
            "origin_name": route_source["origin"],
            "origin_source_owner": route_source["origin_source_owner"],
            "route_count": len(entries),
        }
    )
    effect_origin_route_artifacts.append(row)

assert global_action_tag == 136
assert len(ceremony_symbol_artifacts) == 11
assert len(effect_origin_route_artifacts) == 23
assert route_entry_count == 79


dag_edges = [
    [1, 2, 1],
    [2, 5, 1],
    [3, 5, 2],
    [4, 5, 3],
    [5, 5, 4],
    [6, 6, 1],
    [7, 6, 2],
    [8, 6, 3],
    [9, 6, 4],
    [10, 6, 5],
    [11, 7, 1],
    [12, 7, 2],
    [13, 7, 3],
    [14, 7, 5],
    [15, 7, 6],
    [16, 8, 1],
    [17, 8, 2],
    [18, 8, 4],
    [19, 8, 5],
    [20, 8, 6],
    [21, 9, 1],
    [22, 9, 2],
    [23, 9, 3],
    [24, 9, 4],
    [25, 9, 5],
    [26, 9, 6],
    [27, 9, 7],
    [28, 9, 8],
]

grammar_value = [
    1,
    b32(dictionaries["CatalogTagV1"]["dictionary_id"]),
    b32(owner_dictionary_id),
    b32(profile_dictionary_id),
    [exact_schema_ref_value(i + 1, name) for i, name in enumerate(schemas)],
    [
        [1, 1, b32(profile_sets["OwnerProfileSetV1"]["profile_set_id"])],
        [2, 2, b32(profile_sets["ProtocolProfileSetV1"]["profile_set_id"])],
        [3, 3, b32(profile_sets["PolicyProfileSetV1"]["profile_set_id"])],
    ],
    dag_edges,
    13,
    len(schemas),
    20,
    6,
    6,
    [row["value"] for row in action_symbol_artifacts],
    [row["value"] for row in ceremony_symbol_artifacts],
    [row["value"] for row in effect_origin_route_artifacts],
    [
        clause(1, "no_secret_or_authority", "Profiles contain no secret, credential, protected topology, mutable selector, request, Receipt, Result, current Head, or runtime authority."),
        clause(2, "later_nominal_schemas", "Each catalog Decision publishes literal catalog-specific descriptor, row, HeaderV1, generated closed sum, and ManifestValueV1 schemas plus all values, bytes, IDs, bounds, profiles, and receipts."),
        clause(3, "inline_route_symbols", "Every route content-binds an exact non-executable ActionLeafSymbolV1 or CeremonySymbolV1 identity rather than a future Manifest identifier; later catalogs prove equality and no symbol independently authorizes execution."),
        clause(4, "exact_route_closure", "The grammar contains exactly twenty-three ordered EffectOriginRouteV1 values and seventy-nine route entries, with route-local role, context and basis. Empty, duplicate, unknown, phase-incomplete and cross-context routes fail closed."),
        clause(5, "exact_symbol_universe", "The grammar contains exactly 136 dense Action symbols and eleven dense Ceremony symbols; OwnerProfile membership projects all 147 symbols exactly once and grants no runtime authority."),
    ],
]
grammar_envelope = [
    "maestro.vnext.catalog-profile-grammar.v1",
    b32(schemas["maestro.vnext.catalog.profile-grammar-value.v1"]["schema_id"]),
    grammar_value,
]
grammar_id, grammar_hex, grammar_len = digest(grammar_envelope)


artifact = {
    "status": "candidate_contract_component",
    "schema_version": "maestro.vnext.catalog-profile-grammar.artifact.v1",
    "nominal_source_sha256": hashlib.sha256(SOURCE_PATH.read_bytes()).hexdigest(),
    "decision_inputs": SOURCE["decision_inputs"],
    "dictionaries": dictionaries,
      "owner_dictionary": {
        "value": owner_dictionary_value,
        "dictionary_id": owner_dictionary_id,
          "identity_envelope": [
              "maestro.vnext.catalog.owner-dictionary.v1",
              b32(schemas["maestro.vnext.catalog.owner-dictionary.v1"]["schema_id"]),
              owner_dictionary_value,
          ],
        "cbor_hex": owner_dictionary_hex,
        "byte_length": owner_dictionary_len,
        "sha256": owner_dictionary_id,
    },
      "profile_dictionary": {
        "value": profile_dictionary_value,
        "dictionary_id": profile_dictionary_id,
          "identity_envelope": [
              "maestro.vnext.catalog.profile-dictionary.v1",
              b32(schemas["maestro.vnext.catalog.profile-dictionary.v1"]["schema_id"]),
              profile_dictionary_value,
          ],
        "cbor_hex": profile_dictionary_hex,
        "byte_length": profile_dictionary_len,
        "sha256": profile_dictionary_id,
    },
    "schemas": schemas,
    "owner_profiles": owner_artifacts,
    "protocol_profiles": protocol_artifacts,
    "policy_profiles": policy_artifacts,
    "profile_sets": profile_sets,
    "action_leaf_symbols": action_symbol_artifacts,
    "ceremony_symbols": ceremony_symbol_artifacts,
    "effect_origin_routes": effect_origin_route_artifacts,
    "effect_origin_route_entry_count": route_entry_count,
    "dependency_dag_edges": dag_edges,
    "catalog_profile_grammar": {
        "value": grammar_value,
        "catalog_profile_grammar_id": grammar_id,
        "identity_envelope": grammar_envelope,
        "cbor_hex": grammar_hex,
        "byte_length": grammar_len,
        "sha256": grammar_id,
    },
}


identity_inputs = []
for row in dictionaries.values():
    identity_inputs.append(row["identity_envelope"])
identity_inputs.extend(
    [
        artifact["owner_dictionary"]["identity_envelope"],
        artifact["profile_dictionary"]["identity_envelope"],
    ]
)
identity_inputs.extend(row["identity_envelope"] for row in schemas.values())
identity_inputs.extend(row["identity_envelope"] for row in owner_artifacts)
identity_inputs.extend(row["identity_envelope"] for row in protocol_artifacts)
identity_inputs.extend(row["identity_envelope"] for row in policy_artifacts)
identity_inputs.extend(row["identity_envelope"] for row in profile_sets.values())
identity_inputs.extend(row["identity_envelope"] for row in action_symbol_artifacts)
identity_inputs.extend(row["identity_envelope"] for row in ceremony_symbol_artifacts)
identity_inputs.extend(row["identity_envelope"] for row in effect_origin_route_artifacts)
identity_inputs.append(grammar_envelope)

artifact_path = ROOT / "vnext-catalog-profile-grammar-v1.json"
input_path = ROOT / "vnext-catalog-profile-grammar-v1-encoder-input.json"
text_path = ROOT / "vnext-catalog-profile-grammar-v1.txt"

artifact_bytes = (json.dumps(artifact, indent=2, sort_keys=True) + "\n").encode("ascii")
immutable_artifact_path = ROOT / (
    "vnext-catalog-profile-grammar-v1-sha256-" + grammar_id + ".json"
)
input_bytes = (json.dumps(identity_inputs, separators=(",", ":")) + "\n").encode("ascii")
candidate_artifact_path = ROOT / ".vnext-catalog-profile-grammar-v1.candidate.json"
candidate_input_path = ROOT / ".vnext-catalog-profile-grammar-v1-encoder-input.candidate.json"
candidate_artifact_path.write_bytes(artifact_bytes)
candidate_input_path.write_bytes(input_bytes)


def encoder_receipt(command: list[str]):
    result = subprocess.run(command, check=True, capture_output=True, text=True)
    lines = result.stdout.strip().splitlines()
    if len(lines) != 3:
        raise RuntimeError(result.stdout)
    return {"cbor_hex": lines[0], "byte_length": int(lines[1]), "sha256": lines[2]}


python_receipt = encoder_receipt(
    ["python3", str(ROOT / "vnext_manifest_encode_py.py"), str(candidate_input_path)]
)
ruby_receipt = encoder_receipt(
    ["ruby", str(ROOT / "vnext_manifest_encode_rb.rb"), str(candidate_input_path)]
)
aggregate_id, aggregate_hex, aggregate_len = digest(identity_inputs)
if python_receipt != ruby_receipt:
    raise RuntimeError("independent encoder receipts disagree")
if python_receipt != {
    "cbor_hex": aggregate_hex,
    "byte_length": aggregate_len,
    "sha256": aggregate_id,
}:
    raise RuntimeError("builder and independent encoder receipts disagree")

input_sha256 = hashlib.sha256(input_bytes).hexdigest()
artifact_file_sha256 = hashlib.sha256(artifact_bytes).hexdigest()
python_encoder_sha256 = hashlib.sha256((ROOT / "vnext_manifest_encode_py.py").read_bytes()).hexdigest()
ruby_encoder_sha256 = hashlib.sha256((ROOT / "vnext_manifest_encode_rb.rb").read_bytes()).hexdigest()

validator_path = ROOT / "vnext_catalog_profile_grammar_validate.py"
if not validator_path.exists():
    raise RuntimeError("independent semantic validator is required")
validator_result = subprocess.run(
    ["python3", str(validator_path), str(candidate_artifact_path), str(SOURCE_PATH)],
    check=True,
    capture_output=True,
    text=True,
)
validator_receipt = json.loads(validator_result.stdout.strip())
validator_sha256 = hashlib.sha256(validator_path.read_bytes()).hexdigest()

if immutable_artifact_path.exists():
    if immutable_artifact_path.read_bytes() != artifact_bytes:
        raise RuntimeError("immutable grammar identity path already exists with different bytes")
else:
    with immutable_artifact_path.open("xb") as file:
        file.write(artifact_bytes)
artifact_path.write_bytes(artifact_bytes)
input_path.write_bytes(input_bytes)
candidate_artifact_path.unlink()
candidate_input_path.unlink()

text_lines = [
    "Choose CatalogProfileGrammarV1 as the strict content-bound prerequisite under dec-canonical-manifestidentityv1-byte-729e; do not supersede 729e.",
    "",
    "CatalogProfileGrammarV1Id: sha256:" + grammar_id,
    "CatalogProfileGrammarV1 CBOR bytes: " + grammar_hex,
    "CatalogProfileGrammarV1 byte length: " + str(grammar_len),
    "Immutable artifact: " + immutable_artifact_path.name,
    "Immutable artifact file SHA-256: " + artifact_file_sha256,
    "Nominal source SHA-256: " + artifact["nominal_source_sha256"],
    "",
    "The complete literal artifact below is normative candidate Contract material. Profiles and descriptors are compile-time only and never runtime interpreters, registries, authority, currentness, or extension points.",
    "",
    "```json",
    json.dumps(artifact, indent=2, sort_keys=True),
    "```",
    "",
    "Independent encoder equality receipts:",
    "- Encoder input SHA-256: " + input_sha256,
    "- Python encoder SHA-256: " + python_encoder_sha256,
    "- Ruby encoder SHA-256: " + ruby_encoder_sha256,
    "- Semantic validator SHA-256: " + validator_sha256,
    "- Semantic mutants rejected: " + str(validator_receipt["mutants_rejected"]),
    "- Python aggregate canonical-CBOR byte length: " + str(python_receipt["byte_length"]),
    "- Ruby aggregate canonical-CBOR byte length: " + str(ruby_receipt["byte_length"]),
    "- Python aggregate SHA-256: " + python_receipt["sha256"],
    "- Ruby aggregate SHA-256: " + ruby_receipt["sha256"],
    "- Equality result: exact bytes, length and digest match over all " + str(len(identity_inputs)) + " literal identity envelopes.",
]
text_path.write_text("\n".join(text_lines) + "\n", encoding="ascii")

print(json.dumps({
    "artifact": str(artifact_path),
    "immutable_artifact": str(immutable_artifact_path),
    "immutable_artifact_file_sha256": artifact_file_sha256,
    "encoder_input": str(input_path),
    "decision_text": str(text_path),
    "catalog_profile_grammar_id": grammar_id,
    "aggregate_sha256": aggregate_id,
    "aggregate_byte_length": aggregate_len,
    "identity_input_count": len(identity_inputs),
    "route_entry_count": route_entry_count,
    "semantic_validator_sha256": validator_sha256,
    "semantic_mutants_rejected": validator_receipt["mutants_rejected"],
}, indent=2))
