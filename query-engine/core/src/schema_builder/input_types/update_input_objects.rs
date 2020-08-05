use super::*;

/// Builds "<x>UpdateInput" input object type.
pub(crate) fn update_input_type(ctx: &mut BuilderContext, model: &ModelRef) -> InputObjectTypeWeakRef {
    let name = format!("{}UpdateInput", model.name);
    return_cached_input!(ctx, &name);

    let input_object = Arc::new(init_input_object_type(name.clone()));
    ctx.cache_input_type(name, input_object.clone());

    // Compute input fields for scalar fields.
    let mut fields = scalar_input_fields_for_update(ctx, model);

    // Compute input fields for relational fields.
    let mut relational_fields = relation_input_fields_for_update(ctx, model, None);
    fields.append(&mut relational_fields);

    input_object.set_fields(fields);
    Arc::downgrade(&input_object)
}

/// Builds "<x>UpdateManyMutationInput" input object type.
pub(crate) fn update_many_input_type(ctx: &mut BuilderContext, model: &ModelRef) -> InputObjectTypeWeakRef {
    let object_name = format!("{}UpdateManyMutationInput", model.name);
    return_cached_input!(ctx, &object_name);

    let input_fields = scalar_input_fields_for_update(ctx, model);
    let input_object = Arc::new(input_object_type(object_name.clone(), input_fields));

    ctx.cache_input_type(object_name, input_object.clone());
    Arc::downgrade(&input_object)
}

fn scalar_input_fields_for_update(ctx: &mut BuilderContext, model: &ModelRef) -> Vec<InputField> {
    input_fields::scalar_input_fields(
        ctx,
        model.name.clone(),
        "Update",
        model
            .fields()
            .scalar_writable()
            .filter(field_should_be_kept_for_update_input_type)
            .collect(),
        |f: ScalarFieldRef| map_optional_input_type(&f),
        false,
    )
}

/// For update input types only. Compute input fields for relational fields.
/// This recurses into create_input_type (via nested_create_input_field).
/// Todo: This code is fairly similar to "create" relation computation. Let's see if we can dry it up.
fn relation_input_fields_for_update(
    ctx: &mut BuilderContext,
    model: &ModelRef,
    parent_field: Option<&RelationFieldRef>,
) -> Vec<InputField> {
    model
        .fields()
        .relation()
        .iter()
        .filter_map(|rf| {
            let related_model = rf.related_model();
            let related_field = rf.related_field();

            // Compute input object name
            let arity_part = match (rf.is_list, rf.is_required) {
                (true, _) => "Many",
                (false, true) => "OneRequired",
                (false, false) => "One",
            };

            let without_part = format!("Without{}", capitalize(&related_field.name));

            let input_name = format!("{}Update{}{}Input", related_model.name, arity_part, without_part);
            let field_is_opposite_relation_field =
                parent_field.filter(|pf| pf.related_field().name == rf.name).is_some();

            if field_is_opposite_relation_field {
                None
            } else {
                let input_object = match ctx.get_input_type(&input_name) {
                    Some(t) => t,
                    None => {
                        let input_object = Arc::new(init_input_object_type(input_name.clone()));
                        ctx.cache_input_type(input_name, input_object.clone());

                        let mut fields = vec![input_fields::nested_create_input_field(ctx, rf)];

                        append_opt(&mut fields, input_fields::nested_connect_input_field(ctx, rf));
                        append_opt(&mut fields, input_fields::nested_set_input_field(ctx, rf));
                        append_opt(&mut fields, input_fields::nested_disconnect_input_field(ctx, rf));
                        append_opt(&mut fields, input_fields::nested_delete_input_field(ctx, rf));
                        fields.push(input_fields::nested_update_input_field(ctx, rf));
                        append_opt(&mut fields, input_fields::nested_update_many_field(ctx, rf));
                        append_opt(&mut fields, input_fields::nested_delete_many_field(ctx, rf));
                        append_opt(&mut fields, input_fields::nested_upsert_field(ctx, rf));

                        if feature_flags::get().connectOrCreate {
                            append_opt(&mut fields, input_fields::nested_connect_or_create_field(ctx, rf));
                        }

                        input_object.set_fields(fields);
                        Arc::downgrade(&input_object)
                    }
                };

                let field_type = InputType::opt(InputType::object(input_object));

                Some(input_field(rf.name.clone(), field_type, None))
            }
        })
        .collect()
}

pub(crate) fn nested_upsert_input_object(
    ctx: &mut BuilderContext,
    parent_field: &RelationFieldRef,
) -> Option<InputObjectTypeWeakRef> {
    let nested_update_data_object = nested_update_data(ctx, parent_field);

    if parent_field.is_list {
        nested_upsert_list_input_object(ctx, parent_field, nested_update_data_object)
    } else {
        nested_upsert_nonlist_input_object(ctx, parent_field, nested_update_data_object)
    }
}

/// Builds "<x>UpsertWithWhereUniqueNestedInput" / "<x>UpsertWithWhereUniqueWithout<y>Input" input object types.
fn nested_upsert_list_input_object(
    ctx: &mut BuilderContext,
    parent_field: &RelationFieldRef,
    update_object: InputObjectTypeWeakRef,
) -> Option<InputObjectTypeWeakRef> {
    let related_model = parent_field.related_model();
    let where_object = filter_input_objects::where_unique_object_type(ctx, &related_model);
    let create_object = create_input_objects::create_input_type(ctx, &related_model, Some(parent_field));

    if where_object.into_arc().is_empty() || create_object.into_arc().is_empty() {
        return None;
    }

    let type_name = format!(
        "{}UpsertWithWhereUniqueWithout{}Input",
        related_model.name,
        capitalize(&parent_field.related_field().name)
    );

    match ctx.get_input_type(&type_name) {
        None => {
            let input_object = Arc::new(init_input_object_type(type_name.clone()));
            ctx.cache_input_type(type_name, input_object.clone());

            let fields = vec![
                input_field("where", InputType::object(where_object), None),
                input_field("update", InputType::object(update_object), None),
                input_field("create", InputType::object(create_object), None),
            ];

            input_object.set_fields(fields);
            Some(Arc::downgrade(&input_object))
        }
        x => x,
    }
}

/// Builds "<x>UpsertNestedInput" / "<x>UpsertWithout<y>Input" input object types.
fn nested_upsert_nonlist_input_object(
    ctx: &mut BuilderContext,
    parent_field: &RelationFieldRef,
    update_object: InputObjectTypeWeakRef,
) -> Option<InputObjectTypeWeakRef> {
    let related_model = parent_field.related_model();
    let create_object = create_input_objects::create_input_type(ctx, &related_model, Some(parent_field));

    if create_object.into_arc().is_empty() {
        return None;
    }

    let type_name = format!(
        "{}UpsertWithout{}Input",
        related_model.name,
        capitalize(&parent_field.related_field().name)
    );

    match ctx.get_input_type(&type_name) {
        None => {
            let input_object = Arc::new(init_input_object_type(type_name.clone()));
            ctx.cache_input_type(type_name, input_object.clone());

            let fields = vec![
                input_field("update", InputType::object(update_object), None),
                input_field("create", InputType::object(create_object), None),
            ];

            input_object.set_fields(fields);
            Some(Arc::downgrade(&input_object))
        }
        x => x,
    }
}

/// Builds "<x>UpdateManyWithWhereNestedInput" input object type.
pub(crate) fn nested_update_many_input_object(
    ctx: &mut BuilderContext,
    field: &RelationFieldRef,
) -> Option<InputObjectTypeWeakRef> {
    if field.is_list {
        let related_model = field.related_model();
        let type_name = format!("{}UpdateManyWithWhereNestedInput", related_model.name);

        match ctx.get_input_type(&type_name) {
            None => {
                let data_input_object = nested_update_many_data(ctx, field);
                let input_object = Arc::new(init_input_object_type(type_name.clone()));
                ctx.cache_input_type(type_name, input_object.clone());

                let where_input_object = filter_input_objects::scalar_filter_object_type(ctx, &related_model);

                input_object.set_fields(vec![
                    input_field("where", InputType::object(where_input_object), None),
                    input_field("data", InputType::object(data_input_object), None),
                ]);

                Some(Arc::downgrade(&input_object))
            }
            x => x,
        }
    } else {
        None
    }
}

/// Builds "<x>UpdateWithWhereUniqueNestedInput" / "<x>UpdateWithWhereUniqueWithout<y>Input" input object types.
pub(crate) fn input_object_type_nested_update(
    ctx: &mut BuilderContext,
    parent_field: &RelationFieldRef,
) -> InputObjectTypeWeakRef {
    let related_model = parent_field.related_model();
    let nested_input_object = nested_update_data(ctx, parent_field);

    if parent_field.is_list {
        let where_input_object = filter_input_objects::where_unique_object_type(ctx, &related_model);
        let type_name = format!(
            "{}UpdateWithWhereUniqueWithout{}Input",
            related_model.name,
            capitalize(&parent_field.related_field().name)
        );

        return_cached_input!(ctx, &type_name);
        let input_object = Arc::new(init_input_object_type(type_name.clone()));
        ctx.cache_input_type(type_name, input_object.clone());

        let fields = vec![
            input_field("where", InputType::object(where_input_object), None),
            input_field("data", InputType::object(nested_input_object), None),
        ];

        input_object.set_fields(fields);
        Arc::downgrade(&input_object)
    } else {
        nested_input_object
    }
}

/// Builds "<x>UpdateDataInput" / "<x>UpdateWithout<y>DataInput" ubout input object types.
fn nested_update_data(ctx: &mut BuilderContext, parent_field: &RelationFieldRef) -> InputObjectTypeWeakRef {
    let related_model = parent_field.related_model();
    let type_name = format!(
        "{}UpdateWithout{}DataInput",
        related_model.name,
        capitalize(&parent_field.related_field().name)
    );

    return_cached_input!(ctx, &type_name);

    let input_object = Arc::new(init_input_object_type(&type_name));
    ctx.cache_input_type(type_name, input_object.clone());

    let mut fields = scalar_input_fields_for_update(ctx, &related_model);
    let mut relational_input_fields = relation_input_fields_for_update(ctx, &related_model, Some(parent_field));

    fields.append(&mut relational_input_fields);
    input_object.set_fields(fields);

    Arc::downgrade(&input_object)
}

/// Builds "<x>UpdateManyDataInput" input object type.
fn nested_update_many_data(ctx: &mut BuilderContext, parent_field: &RelationFieldRef) -> InputObjectTypeWeakRef {
    let related_model = parent_field.related_model();
    let type_name = format!("{}UpdateManyDataInput", related_model.name);

    return_cached_input!(ctx, &type_name);

    let input_object = Arc::new(init_input_object_type(type_name.clone()));
    ctx.cache_input_type(type_name, input_object.clone());

    let fields = scalar_input_fields_for_update(ctx, &related_model);

    input_object.set_fields(fields);
    Arc::downgrade(&input_object)
}

fn field_should_be_kept_for_update_input_type(field: &ScalarFieldRef) -> bool {
    // We forbid updating auto-increment integer unique fields as this can create problems with the
    // underlying sequences.
    !field.is_auto_generated_int_id
        && !matches!(
            (&field.type_identifier, field.unique(), field.is_autoincrement),
            (TypeIdentifier::Int, true, true)
        )
}