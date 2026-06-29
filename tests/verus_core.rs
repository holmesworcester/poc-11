use linktoy_verus_core::{
    project_fact_core, AdmittedFactCore, FieldCore, ValidatedOfferCore, ValidityCore,
};

#[test]
fn valid_ready_fact_promotes_only_own_offers_and_fields() {
    let fact = AdmittedFactCore {
        id: 10,
        needs: vec![1, 2],
        offers: vec![10, 11],
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
        ValidatedOfferCore { owner: 1, key: 1 },
        ValidatedOfferCore { owner: 2, key: 2 },
    ];

    let plan = project_fact_core(&fact, &ctx, ValidityCore::Valid);

    assert!(plan.valid);
    assert_eq!(plan.promoted_offers.len(), 2);
    assert_eq!(plan.promoted_offers[0].owner, 10);
    assert_eq!(plan.promoted_offers[0].key, 10);
    assert_eq!(plan.promoted_offers[1].owner, 10);
    assert_eq!(plan.promoted_offers[1].key, 11);
    assert_eq!(plan.promoted_fields.len(), 2);
    assert_eq!(plan.promoted_fields[0].owner, 10);
    assert_eq!(plan.promoted_fields[0].field.name, 100);
    assert_eq!(plan.promoted_fields[0].field.value, 200);
}

#[test]
fn valid_but_not_ready_fact_promotes_nothing() {
    let fact = AdmittedFactCore {
        id: 10,
        needs: vec![1, 2],
        offers: vec![10],
        fields: vec![FieldCore {
            name: 100,
            value: 200,
        }],
    };
    let ctx = vec![ValidatedOfferCore { owner: 1, key: 1 }];

    let plan = project_fact_core(&fact, &ctx, ValidityCore::Valid);

    assert!(!plan.valid);
    assert!(plan.promoted_offers.is_empty());
    assert!(plan.promoted_fields.is_empty());
}

#[test]
fn invalid_fact_promotes_nothing_even_when_ready() {
    let fact = AdmittedFactCore {
        id: 10,
        needs: vec![1],
        offers: vec![10],
        fields: vec![FieldCore {
            name: 100,
            value: 200,
        }],
    };
    let ctx = vec![ValidatedOfferCore { owner: 1, key: 1 }];

    let plan = project_fact_core(&fact, &ctx, ValidityCore::Invalid);

    assert!(!plan.valid);
    assert!(plan.promoted_offers.is_empty());
    assert!(plan.promoted_fields.is_empty());
}
