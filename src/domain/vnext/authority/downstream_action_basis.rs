use thiserror::Error;

const DOWNSTREAM_ACTION_FIRST_GLOBAL_TAG_V1: u64 = 94;
const DOWNSTREAM_ACTION_LAST_GLOBAL_TAG_V1: u64 = 145;
const DOWNSTREAM_ACTION_COUNT_V1: usize =
    (DOWNSTREAM_ACTION_LAST_GLOBAL_TAG_V1 - DOWNSTREAM_ACTION_FIRST_GLOBAL_TAG_V1 + 1) as usize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RepositoryDownstreamActionMetadataV1 {
    literal: &'static str,
    global_tag: u64,
    owner_tag: u64,
    local_tag: u64,
    owner_descriptor_id: &'static str,
    descriptor_id: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RepositoryDownstreamActionLeafV1(u8);

impl RepositoryDownstreamActionLeafV1 {
    pub fn all() -> [Self; DOWNSTREAM_ACTION_COUNT_V1] {
        std::array::from_fn(|index| Self::from_catalog_index(index as u8))
    }

    pub(super) const fn from_catalog_index(index: u8) -> Self {
        assert!((index as usize) < DOWNSTREAM_ACTION_COUNT_V1);
        Self(index)
    }

    pub fn from_global_tag(global_tag: u64) -> Result<Self, RepositoryDownstreamActionErrorV1> {
        if !(DOWNSTREAM_ACTION_FIRST_GLOBAL_TAG_V1..=DOWNSTREAM_ACTION_LAST_GLOBAL_TAG_V1)
            .contains(&global_tag)
        {
            return Err(RepositoryDownstreamActionErrorV1::UnknownGlobalTag(
                global_tag,
            ));
        }
        Ok(Self(
            u8::try_from(global_tag - DOWNSTREAM_ACTION_FIRST_GLOBAL_TAG_V1)
                .expect("invariant: downstream Action index is bounded by the frozen catalog"),
        ))
    }

    pub fn parse_exact(literal: &str) -> Result<Self, RepositoryDownstreamActionErrorV1> {
        Self::all()
            .into_iter()
            .find(|action| action.literal() == literal)
            .ok_or_else(|| RepositoryDownstreamActionErrorV1::UnknownLiteral(literal.to_owned()))
    }

    pub const fn literal(self) -> &'static str {
        self.metadata().literal
    }

    pub const fn global_tag(self) -> u64 {
        self.metadata().global_tag
    }

    pub const fn owner_tag(self) -> u64 {
        self.metadata().owner_tag
    }

    pub const fn family_tag(self) -> u64 {
        match self.global_tag() {
            94..=102 => 9,
            103..=106 => 10,
            107..=116 => 11,
            117..=129 => 12,
            130..=131 => 13,
            132..=138 => 14,
            139..=141 => 15,
            142..=145 => 16,
            _ => unreachable!(),
        }
    }

    pub const fn local_tag(self) -> u64 {
        self.metadata().local_tag
    }

    pub const fn owner_descriptor_id(self) -> &'static str {
        self.metadata().owner_descriptor_id
    }

    pub const fn descriptor_id(self) -> &'static str {
        self.metadata().descriptor_id
    }

    const fn metadata(self) -> &'static RepositoryDownstreamActionMetadataV1 {
        &REPOSITORY_DOWNSTREAM_ACTION_METADATA_V1[self.0 as usize]
    }
}

impl TryFrom<u64> for RepositoryDownstreamActionLeafV1 {
    type Error = RepositoryDownstreamActionErrorV1;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Self::from_global_tag(value)
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RepositoryDownstreamActionErrorV1 {
    #[error("global Action tag {0} is outside the frozen downstream Repository range 94..=145")]
    UnknownGlobalTag(u64),
    #[error("Action literal {0} is outside the frozen downstream Repository catalog")]
    UnknownLiteral(String),
}

const REPOSITORY_DOWNSTREAM_ACTION_METADATA_V1: [RepositoryDownstreamActionMetadataV1;
    DOWNSTREAM_ACTION_COUNT_V1] = [
    RepositoryDownstreamActionMetadataV1 {
        literal: "PublishInitialMessage",
        global_tag: 94,
        owner_tag: 10,
        local_tag: 1,
        owner_descriptor_id: "64b1107dc371caae5672679ce7e9829c492231cce09addba73e8c9d86698b8b8",
        descriptor_id: "1de1bbec6ffad10099c0fcc2f0bd87ff948593b7a7dfdedc5798400944e971ec",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "PublishMessage",
        global_tag: 95,
        owner_tag: 10,
        local_tag: 2,
        owner_descriptor_id: "64b1107dc371caae5672679ce7e9829c492231cce09addba73e8c9d86698b8b8",
        descriptor_id: "fcef3d1c4549e2877b7317d4b09f404767ecf0c396434579a03aec32995cd4b8",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "AcknowledgeMessage",
        global_tag: 96,
        owner_tag: 10,
        local_tag: 3,
        owner_descriptor_id: "64b1107dc371caae5672679ce7e9829c492231cce09addba73e8c9d86698b8b8",
        descriptor_id: "b84027432f3ec008c9a7ea6911afe0a01381071174532ab57967668544eaf437",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "ReplaceFocus",
        global_tag: 97,
        owner_tag: 10,
        local_tag: 4,
        owner_descriptor_id: "64b1107dc371caae5672679ce7e9829c492231cce09addba73e8c9d86698b8b8",
        descriptor_id: "b09673b4a24af940eb3dbc0c1aa5e18b351933d2a21f52ef8caab29fbdda5ed2",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "WithdrawFocus",
        global_tag: 98,
        owner_tag: 10,
        local_tag: 5,
        owner_descriptor_id: "64b1107dc371caae5672679ce7e9829c492231cce09addba73e8c9d86698b8b8",
        descriptor_id: "31aa9d59169a67a83ea8791fda7e140bb50dce69e5a278868e4b07d6e162808e",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "PublishScope",
        global_tag: 99,
        owner_tag: 10,
        local_tag: 6,
        owner_descriptor_id: "64b1107dc371caae5672679ce7e9829c492231cce09addba73e8c9d86698b8b8",
        descriptor_id: "10e5f2d07899ddda45aa171a5d40dd09b9cd258b36b79c01848d0b7429639043",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "WithdrawScope",
        global_tag: 100,
        owner_tag: 10,
        local_tag: 7,
        owner_descriptor_id: "64b1107dc371caae5672679ce7e9829c492231cce09addba73e8c9d86698b8b8",
        descriptor_id: "490c68bf9796d3992223249074f9185175efcd207f57bae893d79eaa50dd90d6",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "AssertConflict",
        global_tag: 101,
        owner_tag: 10,
        local_tag: 8,
        owner_descriptor_id: "64b1107dc371caae5672679ce7e9829c492231cce09addba73e8c9d86698b8b8",
        descriptor_id: "d17a766f56371380c0c91c60a68cafdef625dd0e2951e1f4739dda1f09293cf7",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "ResolveConflict",
        global_tag: 102,
        owner_tag: 10,
        local_tag: 9,
        owner_descriptor_id: "64b1107dc371caae5672679ce7e9829c492231cce09addba73e8c9d86698b8b8",
        descriptor_id: "209bcc213ed555d103461b85d307b46b1ac70bd046f7bd52b77e71c13fd7a9ed",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "PublishPlanningProposal",
        global_tag: 103,
        owner_tag: 12,
        local_tag: 1,
        owner_descriptor_id: "a8847b13c1b93bc3eabe6ae2b2bc964ff99b4458bd9fff5c8b910b9a52c9c706",
        descriptor_id: "89a8b0ff821d9a0912c7238f4621f5c9a36a266041f98c547a608bfb43c35012",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "DisposePlanningProposal",
        global_tag: 104,
        owner_tag: 12,
        local_tag: 2,
        owner_descriptor_id: "a8847b13c1b93bc3eabe6ae2b2bc964ff99b4458bd9fff5c8b910b9a52c9c706",
        descriptor_id: "6effd71ed2611f5fb5835ecd554b612fa151d899ef4eafe19dd45b0845de144c",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "PublishSchedulingPolicyBinding",
        global_tag: 105,
        owner_tag: 12,
        local_tag: 3,
        owner_descriptor_id: "a8847b13c1b93bc3eabe6ae2b2bc964ff99b4458bd9fff5c8b910b9a52c9c706",
        descriptor_id: "51b9e80955ce454bfa178234cc61831559307d5c01b388562a25cdf9668d08c2",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "PublishSchedulingAssessment",
        global_tag: 106,
        owner_tag: 12,
        local_tag: 4,
        owner_descriptor_id: "a8847b13c1b93bc3eabe6ae2b2bc964ff99b4458bd9fff5c8b910b9a52c9c706",
        descriptor_id: "e4555455ab82afc1a1749c71cb9b49fb90a49d50d93f6b1e074e589a170459d5",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "PublishLogicalTombstone",
        global_tag: 107,
        owner_tag: 14,
        local_tag: 1,
        owner_descriptor_id: "0cdc8d378a24916c19712f7f20996c17b83d48ac90b3c304b8dc917394eb019a",
        descriptor_id: "4c7c62eaba9cd1017dee7dbea45a158ac4c87b772ad0b01d4fec40de5dc9570e",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "PublishSecurityErasureObligation",
        global_tag: 108,
        owner_tag: 14,
        local_tag: 2,
        owner_descriptor_id: "0cdc8d378a24916c19712f7f20996c17b83d48ac90b3c304b8dc917394eb019a",
        descriptor_id: "3e8b2340e581a90b8ed3954aab2125c3e2fa995320da5eb6f25dad249d5bcf08",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "ExecuteGcSweep",
        global_tag: 109,
        owner_tag: 14,
        local_tag: 3,
        owner_descriptor_id: "0cdc8d378a24916c19712f7f20996c17b83d48ac90b3c304b8dc917394eb019a",
        descriptor_id: "70b62bdd12bad69a430d50bfa222cd1081a2ac3f790c52ad72db9a9da597869c",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "CreateSealedExport",
        global_tag: 110,
        owner_tag: 14,
        local_tag: 4,
        owner_descriptor_id: "0cdc8d378a24916c19712f7f20996c17b83d48ac90b3c304b8dc917394eb019a",
        descriptor_id: "796b178569c14fb9ea79f2f650c745e492ca57c9fa1cac040c88740b80d1fbd9",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "VerifyBackup",
        global_tag: 111,
        owner_tag: 14,
        local_tag: 5,
        owner_descriptor_id: "0cdc8d378a24916c19712f7f20996c17b83d48ac90b3c304b8dc917394eb019a",
        descriptor_id: "fb0b0a77c427edf09ec29433ffa8f3ba1d479eebe88442811c1df083bebed32c",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "StageRestoreCandidate",
        global_tag: 112,
        owner_tag: 14,
        local_tag: 6,
        owner_descriptor_id: "0cdc8d378a24916c19712f7f20996c17b83d48ac90b3c304b8dc917394eb019a",
        descriptor_id: "f51dda52f43ce1d25dfc548fb364c91796b4bfcae4422c1129193653a27a7984",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "VerifyRestoreCandidate",
        global_tag: 113,
        owner_tag: 14,
        local_tag: 7,
        owner_descriptor_id: "0cdc8d378a24916c19712f7f20996c17b83d48ac90b3c304b8dc917394eb019a",
        descriptor_id: "edb803b6648bf2db70cdcb39c70b8ec643c3bcff074f6f2c84e9fe7a82fe4f2d",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "ReconcileAppendableHistory",
        global_tag: 114,
        owner_tag: 14,
        local_tag: 8,
        owner_descriptor_id: "0cdc8d378a24916c19712f7f20996c17b83d48ac90b3c304b8dc917394eb019a",
        descriptor_id: "a286444e3c6b378b9ab4b3511dff8f59a94e1f586f13b476442ceff880ecf353",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "StageGovernedReviewPublicationClosure",
        global_tag: 115,
        owner_tag: 14,
        local_tag: 9,
        owner_descriptor_id: "0cdc8d378a24916c19712f7f20996c17b83d48ac90b3c304b8dc917394eb019a",
        descriptor_id: "0fc918f673de664b89ab6fc372efdff19bd6989acfcdde7c5a9828b0c0a2e4ba",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "TombstoneGovernedReviewPublicationClosure",
        global_tag: 116,
        owner_tag: 14,
        local_tag: 10,
        owner_descriptor_id: "0cdc8d378a24916c19712f7f20996c17b83d48ac90b3c304b8dc917394eb019a",
        descriptor_id: "ea12d3b5f0188e6f98a21ce4562dfe97f3aa0edd7810610ac9cdc20536bcf6e0",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "ReserveDistributionTargets",
        global_tag: 117,
        owner_tag: 20,
        local_tag: 1,
        owner_descriptor_id: "bea1640f4cc46a75e7e295eb555927201121ae69a7f59063e3a0d6708e1292e2",
        descriptor_id: "bc76796a22070cb8fb343db3db372a49466dca1d20a7f8c8ce937b804b921b19",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "AdoptManagedRegion",
        global_tag: 118,
        owner_tag: 20,
        local_tag: 2,
        owner_descriptor_id: "bea1640f4cc46a75e7e295eb555927201121ae69a7f59063e3a0d6708e1292e2",
        descriptor_id: "b177e658a470818c2a53b8320da3bb0bb6bb19ab9bc6c53d02dbdf741671b237",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "TransferWholeFileCustody",
        global_tag: 119,
        owner_tag: 20,
        local_tag: 3,
        owner_descriptor_id: "bea1640f4cc46a75e7e295eb555927201121ae69a7f59063e3a0d6708e1292e2",
        descriptor_id: "f5c1eadeed317406bb964e308ac1b2d7dfae486835e4c9f39b2175655df2b708",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "BeginDistributionTransaction",
        global_tag: 120,
        owner_tag: 20,
        local_tag: 4,
        owner_descriptor_id: "bea1640f4cc46a75e7e295eb555927201121ae69a7f59063e3a0d6708e1292e2",
        descriptor_id: "eb0e7e41678e1218a2b681a9a63852fd6943638a6283ecbbc3f9f8dc0d45a6db",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "CaptureDistributionBeforeState",
        global_tag: 121,
        owner_tag: 20,
        local_tag: 5,
        owner_descriptor_id: "bea1640f4cc46a75e7e295eb555927201121ae69a7f59063e3a0d6708e1292e2",
        descriptor_id: "e064258d290cb25d72c4a3ecbe14caf7667b3138914bb779e79e4250686df91e",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "StageDistributionCandidate",
        global_tag: 122,
        owner_tag: 20,
        local_tag: 6,
        owner_descriptor_id: "bea1640f4cc46a75e7e295eb555927201121ae69a7f59063e3a0d6708e1292e2",
        descriptor_id: "0d0719afecf607ffc6eb501ed44c708b287f723c939a40d82957aac744d03e88",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "ReserveDistributionEffect",
        global_tag: 123,
        owner_tag: 20,
        local_tag: 7,
        owner_descriptor_id: "bea1640f4cc46a75e7e295eb555927201121ae69a7f59063e3a0d6708e1292e2",
        descriptor_id: "7c90d933fe22345895ec04e9fe9b32061839c5a68aa42c46676c1d53249e8d13",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "PublishDistributionOccurrence",
        global_tag: 124,
        owner_tag: 20,
        local_tag: 8,
        owner_descriptor_id: "bea1640f4cc46a75e7e295eb555927201121ae69a7f59063e3a0d6708e1292e2",
        descriptor_id: "a83398caf01d4a0c9d5f2b42a71f4b74abd6b73b4763ba7232a83d0f01be98ab",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "VerifyDistributionTarget",
        global_tag: 125,
        owner_tag: 20,
        local_tag: 9,
        owner_descriptor_id: "bea1640f4cc46a75e7e295eb555927201121ae69a7f59063e3a0d6708e1292e2",
        descriptor_id: "b5424286e0fbee75cc164d22e749896f51a57cad81b9d9a544c3decb36262fbf",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "CommitDistributionTransaction",
        global_tag: 126,
        owner_tag: 20,
        local_tag: 10,
        owner_descriptor_id: "bea1640f4cc46a75e7e295eb555927201121ae69a7f59063e3a0d6708e1292e2",
        descriptor_id: "f571d2fdf0e37b959fdf41651f79b086fc3a3ccea5767cbc45ec833bd3026531",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "RecoverDistributionTransaction",
        global_tag: 127,
        owner_tag: 20,
        local_tag: 11,
        owner_descriptor_id: "bea1640f4cc46a75e7e295eb555927201121ae69a7f59063e3a0d6708e1292e2",
        descriptor_id: "a39fff81e6ba55bf79bcb8c0002cd8d1acd406bc9a95aa700f23f0cd68bfed7f",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "RollbackDistributionTransaction",
        global_tag: 128,
        owner_tag: 20,
        local_tag: 12,
        owner_descriptor_id: "bea1640f4cc46a75e7e295eb555927201121ae69a7f59063e3a0d6708e1292e2",
        descriptor_id: "8e07516549d8e73405f63ab75a9e835477e758a6b84e2282b9b03a92d84ac11f",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "ActivateBinarySlot",
        global_tag: 129,
        owner_tag: 20,
        local_tag: 13,
        owner_descriptor_id: "bea1640f4cc46a75e7e295eb555927201121ae69a7f59063e3a0d6708e1292e2",
        descriptor_id: "d2e6ce9efda2b399a9c9457bb151e6494bfb2b48d14c4ec8fc6c52c0feecb637",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "RebuildSearchIndex",
        global_tag: 130,
        owner_tag: 15,
        local_tag: 1,
        owner_descriptor_id: "39c86195e6fdebbfe87257869a0143e875b2260a4b35c3a706ee7855af91fdc2",
        descriptor_id: "d04e070bb70691d8618102e87a9b68435ea7b38fb6ce6bb68cb7c6c771393c0a",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "PurgeSearchIndex",
        global_tag: 131,
        owner_tag: 15,
        local_tag: 2,
        owner_descriptor_id: "39c86195e6fdebbfe87257869a0143e875b2260a4b35c3a706ee7855af91fdc2",
        descriptor_id: "5082eea1acee372b5c872e95d0026ce3c4a5dbf346cef59434ad3f59d4218150",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "CreateMemoryCandidate",
        global_tag: 132,
        owner_tag: 16,
        local_tag: 1,
        owner_descriptor_id: "d869f02709a56087cfe1ef51d94c870a7310a622a922c0d830075b8f9d0442fa",
        descriptor_id: "80a65b49e5b8b678bee4cd9964d66d2f867605698c8f5c4ff417970de4052fd2",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "PromoteMemoryCandidate",
        global_tag: 133,
        owner_tag: 16,
        local_tag: 2,
        owner_descriptor_id: "d869f02709a56087cfe1ef51d94c870a7310a622a922c0d830075b8f9d0442fa",
        descriptor_id: "ad90e157a8e9054ced50575badfbf7e6fa364372a1137df08650b6cca7f3e9eb",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "RejectMemoryCandidate",
        global_tag: 134,
        owner_tag: 16,
        local_tag: 3,
        owner_descriptor_id: "d869f02709a56087cfe1ef51d94c870a7310a622a922c0d830075b8f9d0442fa",
        descriptor_id: "0e7e271037cf46cddae1cadcabb30b20472020a310d4483789b1d3bf4f9f069a",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "QuarantineMemoryCandidate",
        global_tag: 135,
        owner_tag: 16,
        local_tag: 4,
        owner_descriptor_id: "d869f02709a56087cfe1ef51d94c870a7310a622a922c0d830075b8f9d0442fa",
        descriptor_id: "25ad97181f19dc39562175001c71f15d31fa8d080a857021e98231b479b45e17",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "InvalidateMemoryEntry",
        global_tag: 136,
        owner_tag: 16,
        local_tag: 5,
        owner_descriptor_id: "d869f02709a56087cfe1ef51d94c870a7310a622a922c0d830075b8f9d0442fa",
        descriptor_id: "f634872c2b2ec1949d47078ca241ae44a5ca17a9b265dd18e886643329b17c58",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "SupersedeMemoryEntry",
        global_tag: 137,
        owner_tag: 16,
        local_tag: 6,
        owner_descriptor_id: "d869f02709a56087cfe1ef51d94c870a7310a622a922c0d830075b8f9d0442fa",
        descriptor_id: "121741f0bb72322635973a4cf5e1649d154c4767ce0d72ea925f5a6f9be2dba7",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "SecurityEraseMemoryPayload",
        global_tag: 138,
        owner_tag: 16,
        local_tag: 7,
        owner_descriptor_id: "d869f02709a56087cfe1ef51d94c870a7310a622a922c0d830075b8f9d0442fa",
        descriptor_id: "aaed903fcb5768bd5c39feb7e2e3389a0c88cb85153eb9c64f9c80643b02e802",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "RecordIntakeSource",
        global_tag: 139,
        owner_tag: 17,
        local_tag: 1,
        owner_descriptor_id: "3315927ac02d764d1212b3d54b40a917a8806504665e697ebb7d4eb942375725",
        descriptor_id: "a3dd39cedfdc853c081d8cc3538b247899bc7e05c862d94050b25725be87dad6",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "PublishIntakeFinding",
        global_tag: 140,
        owner_tag: 17,
        local_tag: 2,
        owner_descriptor_id: "3315927ac02d764d1212b3d54b40a917a8806504665e697ebb7d4eb942375725",
        descriptor_id: "682bf019626087d3e822331200f274abb14bad3b439a5906159154b5059ae824",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "DisposeIntakeSource",
        global_tag: 141,
        owner_tag: 17,
        local_tag: 3,
        owner_descriptor_id: "3315927ac02d764d1212b3d54b40a917a8806504665e697ebb7d4eb942375725",
        descriptor_id: "507f4da64812ac4c2c9b24a0d77654c98dc6ff13edbd92db7eab577d1fce06b5",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "BeginResearchQuestion",
        global_tag: 142,
        owner_tag: 18,
        local_tag: 1,
        owner_descriptor_id: "75d29ab069b4488d947d4f335eb02fdb5e0777dbcd7de5ca14d8c3d8e3b0af76",
        descriptor_id: "779eb1ef71ec8aaec417930fa2159003c6817be0dbdd4f660679b1bca109f8df",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "AppendResearchQuestionRevision",
        global_tag: 143,
        owner_tag: 18,
        local_tag: 2,
        owner_descriptor_id: "75d29ab069b4488d947d4f335eb02fdb5e0777dbcd7de5ca14d8c3d8e3b0af76",
        descriptor_id: "39574daf5977afd024726e0a00bcf2c26d00720cf44c6b5d097e82bb91106175",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "PublishResearchSynthesis",
        global_tag: 144,
        owner_tag: 18,
        local_tag: 3,
        owner_descriptor_id: "75d29ab069b4488d947d4f335eb02fdb5e0777dbcd7de5ca14d8c3d8e3b0af76",
        descriptor_id: "4776f2ace392e535e16118ee43d2dcc85bfb8446043a3a0fa55c79be94b481a1",
    },
    RepositoryDownstreamActionMetadataV1 {
        literal: "DisposeResearchQuestion",
        global_tag: 145,
        owner_tag: 18,
        local_tag: 4,
        owner_descriptor_id: "75d29ab069b4488d947d4f335eb02fdb5e0777dbcd7de5ca14d8c3d8e3b0af76",
        descriptor_id: "9803450172101ca503f0a55ce390e3a8a3984075c9bbd3a44dacb9a109b9a7bd",
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn downstream_rows_are_the_exact_contiguous_frozen_catalog_slice() {
        let rows = RepositoryDownstreamActionLeafV1::all();
        assert_eq!(rows.len(), 52);
        assert_eq!(rows.first().unwrap().global_tag(), 94);
        assert_eq!(rows.first().unwrap().literal(), "PublishInitialMessage");
        assert_eq!(rows.last().unwrap().global_tag(), 145);
        assert_eq!(rows.last().unwrap().literal(), "DisposeResearchQuestion");
        assert!(rows.windows(2).all(|pair| {
            pair[0].global_tag() + 1 == pair[1].global_tag()
                && RepositoryDownstreamActionLeafV1::parse_exact(pair[0].literal()) == Ok(pair[0])
        }));
    }

    #[test]
    fn downstream_rows_preserve_owner_and_descriptor_bindings() {
        let planning = RepositoryDownstreamActionLeafV1::from_global_tag(103).unwrap();
        assert_eq!(planning.owner_tag(), 12);
        assert_eq!(planning.local_tag(), 1);
        assert_eq!(
            planning.descriptor_id(),
            "89a8b0ff821d9a0912c7238f4621f5c9a36a266041f98c547a608bfb43c35012"
        );
        let distribution = RepositoryDownstreamActionLeafV1::from_global_tag(123).unwrap();
        assert_eq!(distribution.owner_tag(), 20);
        assert_eq!(distribution.local_tag(), 7);
        assert_eq!(
            distribution.owner_descriptor_id(),
            "bea1640f4cc46a75e7e295eb555927201121ae69a7f59063e3a0d6708e1292e2"
        );
        assert!(RepositoryDownstreamActionLeafV1::from_global_tag(93).is_err());
        assert!(RepositoryDownstreamActionLeafV1::from_global_tag(146).is_err());
    }

    #[test]
    fn downstream_rows_match_every_frozen_action_spec_field() {
        let document: Value = serde_json::from_str(include_str!(
            "../../../../contracts/vnext/catalogs/generated/catalog-09-action-spec.json"
        ))
        .unwrap();
        let descriptors = document["descriptors"].as_array().unwrap();
        assert_eq!(descriptors.len(), 145);

        for (action, descriptor) in RepositoryDownstreamActionLeafV1::all()
            .into_iter()
            .zip(&descriptors[93..145])
        {
            let fields = descriptor["value"].as_array().unwrap();
            assert_eq!(fields[0].as_u64(), Some(action.global_tag()));
            assert_eq!(fields[1].as_str(), Some(action.literal()));
            assert_eq!(fields[2][0].as_u64(), Some(action.owner_tag()));
            assert_eq!(
                fields[2][1]["bytes"].as_str(),
                Some(action.owner_descriptor_id())
            );
            assert_eq!(fields[3].as_u64(), Some(action.family_tag()));
            assert_eq!(fields[4].as_u64(), Some(action.local_tag()));
            assert_eq!(
                descriptor["descriptor_id"].as_str(),
                Some(action.descriptor_id())
            );
            assert_eq!(fields[5], serde_json::json!([0]));
            assert_eq!(fields[6].as_u64(), Some(1));
            assert_eq!(fields[7], serde_json::json!([]));
        }
    }
}
