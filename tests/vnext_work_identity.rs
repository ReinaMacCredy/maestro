use maestro::domain::work::{
    WorkIdV1, WorkIdentityError, WorkRelationIdV1, WorkRequirementIdV1, WorkSubmissionIdV1,
};

#[test]
fn work_identity_is_nominal_deterministic_and_canonical() {
    let work = WorkIdV1::derive("work-alpha").unwrap();
    assert_eq!(work, WorkIdV1::derive("work-alpha").unwrap());
    assert_ne!(work, WorkIdV1::derive("work-beta").unwrap());
    assert_eq!(WorkIdV1::parse(&work.render()).unwrap(), work);
    assert_eq!(work.to_string(), work.render());
    assert_eq!(work.into_bytes().len(), 32);

    let rendered = work.render();
    assert!(rendered.starts_with("sha256:"));
    assert_eq!(rendered.len(), 71);
    assert!(
        rendered[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    );

    let same_seed_domains = [
        WorkSubmissionIdV1::derive("work-alpha").unwrap().render(),
        WorkRelationIdV1::derive("work-alpha").unwrap().render(),
        WorkRequirementIdV1::derive("work-alpha").unwrap().render(),
    ];
    assert!(same_seed_domains.iter().all(|other| other != &rendered));
}

#[test]
fn work_identity_refuses_unbounded_non_ascii_or_noncanonical_input() {
    assert_eq!(
        WorkIdV1::derive("").unwrap_err(),
        WorkIdentityError::InvalidSeedLength
    );
    assert_eq!(
        WorkIdV1::derive(&"x".repeat(257)).unwrap_err(),
        WorkIdentityError::InvalidSeedLength
    );
    assert_eq!(
        WorkIdV1::derive("wörk").unwrap_err(),
        WorkIdentityError::InvalidSeedLength
    );

    let rendered = WorkIdV1::derive("work-alpha").unwrap().render();
    assert_eq!(
        WorkIdV1::parse(&rendered.to_uppercase()).unwrap_err(),
        WorkIdentityError::InvalidRenderedIdentity
    );
    assert_eq!(
        WorkIdV1::parse(&rendered[7..]).unwrap_err(),
        WorkIdentityError::InvalidRenderedIdentity
    );
    assert_eq!(
        WorkIdV1::parse("sha256:00").unwrap_err(),
        WorkIdentityError::InvalidRenderedIdentity
    );
}
