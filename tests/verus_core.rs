use linktoy_verus_core::{
    project_fact_core, AdmittedFactCore, Bytes32Core, EdgeAddrCore, FieldCore, ValidatedOfferCore,
    ValidityCore,
};

fn b(n: u64) -> Bytes32Core {
    Bytes32Core {
        w0: n,
        w1: 0,
        w2: 0,
        w3: 0,
    }
}

fn addr(role: u64, key: u64) -> EdgeAddrCore {
    EdgeAddrCore {
        role: b(role),
        scope: 0,
        key: b(key),
    }
}

#[test]
fn valid_ready_fact_promotes_only_own_offers_and_fields() {
    let fact = AdmittedFactCore {
        id: b(10),
        needs: vec![addr(1, 1), addr(1, 2)],
        offers: vec![addr(1, 10), addr(1, 11)],
        fields: vec![
            FieldCore {
                name: 100,
                value: 200,
            },
            FieldCore {
                name: 101,
                value: 201,
            },
        ],
    };
    let ctx = vec![
        ValidatedOfferCore {
            owner: b(1),
            addr: addr(1, 1),
        },
        ValidatedOfferCore {
            owner: b(2),
            addr: addr(1, 2),
        },
    ];

    let plan = project_fact_core(&fact, &ctx, ValidityCore::Valid);

    assert!(plan.valid);
    assert_eq!(plan.promoted_offers.len(), 2);
    assert_eq!(plan.promoted_offers[0].owner, b(10));
    assert_eq!(plan.promoted_offers[0].addr, addr(1, 10));
    assert_eq!(plan.promoted_offers[1].owner, b(10));
    assert_eq!(plan.promoted_offers[1].addr, addr(1, 11));
    assert_eq!(plan.promoted_fields.len(), 2);
    assert_eq!(plan.promoted_fields[0].owner, b(10));
    assert_eq!(plan.promoted_fields[0].field.name, 100);
    assert_eq!(plan.promoted_fields[0].field.value, 200);
}

#[test]
fn valid_but_not_ready_fact_promotes_nothing() {
    let fact = AdmittedFactCore {
        id: b(10),
        needs: vec![addr(1, 1), addr(1, 2)],
        offers: vec![addr(1, 10)],
        fields: vec![FieldCore {
            name: 100,
            value: 200,
        }],
    };
    let ctx = vec![ValidatedOfferCore {
        owner: b(1),
        addr: addr(1, 1),
    }];

    let plan = project_fact_core(&fact, &ctx, ValidityCore::Valid);

    assert!(!plan.valid);
    assert!(plan.promoted_offers.is_empty());
    assert!(plan.promoted_fields.is_empty());
}

#[test]
fn invalid_fact_promotes_nothing_even_when_ready() {
    let fact = AdmittedFactCore {
        id: b(10),
        needs: vec![addr(1, 1)],
        offers: vec![addr(1, 10)],
        fields: vec![FieldCore {
            name: 100,
            value: 200,
        }],
    };
    let ctx = vec![ValidatedOfferCore {
        owner: b(1),
        addr: addr(1, 1),
    }];

    let plan = project_fact_core(&fact, &ctx, ValidityCore::Invalid);

    assert!(!plan.valid);
    assert!(plan.promoted_offers.is_empty());
    assert!(plan.promoted_fields.is_empty());
}

#[test]
fn same_key_with_different_role_does_not_satisfy_need() {
    let fact = AdmittedFactCore {
        id: b(10),
        needs: vec![addr(1, 7)],
        offers: vec![addr(1, 10)],
        fields: vec![],
    };
    let ctx = vec![ValidatedOfferCore {
        owner: b(1),
        addr: addr(2, 7),
    }];

    let plan = project_fact_core(&fact, &ctx, ValidityCore::Valid);

    assert!(!plan.valid);
    assert!(plan.promoted_offers.is_empty());
    assert!(plan.promoted_fields.is_empty());
}
