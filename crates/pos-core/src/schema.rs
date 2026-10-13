// Schema mirrors versioned SQL migrations. Diesel checks ordinary queries against these types.
diesel::table! {
    products (id) {
        id -> Text,
        sku -> Text,
        name -> Text,
        category -> Text,
        price_satang -> BigInt,
        cost_satang -> Nullable<BigInt>,
        stock -> Nullable<BigInt>,
        active -> Bool,
        barcode -> Nullable<Text>,
        on_sale -> Bool,
        options_json -> Text,
        service_charge -> Bool,
        kitchen -> Bool,
    }
}
diesel::table! {
    sales (number) {
        number -> BigInt,
        cart_id -> Text,
        created_at -> BigInt,
        label -> Text,
        total_satang -> BigInt,
        cash_satang -> BigInt,
        change_satang -> BigInt,
        receipt_json -> Text,
        payment_method -> Text,
    }
}
diesel::table! {
    held_carts (id) {
        id -> Text,
        updated_at -> BigInt,
        cart_json -> Text,
    }
}
diesel::table! {
    sale_voids (sale_id) {
        sale_id -> Text,
        id -> Text,
        created_at -> BigInt,
        reason -> Text,
        refund_satang -> BigInt,
        void_json -> Text,
        refund_method -> Text,
    }
}
diesel::table! {
    shifts (id) {
        id -> Text,
        opened_at -> BigInt,
        closed_at -> Nullable<BigInt>,
        shift_json -> Text,
    }
}
diesel::table! {
    cash_movements (id) {
        id -> Text,
        shift_id -> Text,
        created_at -> BigInt,
        movement_json -> Text,
        payment_method -> Text,
    }
}
diesel::table! {
    stock_movements (id) {
        id -> Text,
        product_id -> Text,
        created_at -> BigInt,
        kind -> Text,
        before_qty -> BigInt,
        after_qty -> BigInt,
        reason -> Text,
        staff_json -> Nullable<Text>,
    }
}
diesel::table! {
    staff (id) {
        id -> Text,
        name -> Text,
        role -> Text,
        pin_hash -> Text,
        active -> Integer,
        created_at -> BigInt,
    }
}
diesel::table! {
    roles (id) {
        id -> Text,
        name -> Text,
        role_json -> Text,
    }
}
diesel::table! {
    app_settings (key) {
        key -> Text,
        value -> Text,
    }
}
diesel::table! {
    sale_refunds (id) {
        id -> Text,
        sale_id -> Text,
        created_at -> BigInt,
        reason -> Text,
        refund_satang -> BigInt,
        refund_method -> Text,
        refund_json -> Text,
    }
}
diesel::table! {
    tax_invoices (number) {
        number -> BigInt,
        id -> Text,
        sale_id -> Text,
        created_at -> BigInt,
        invoice_json -> Text,
    }
}
diesel::table! {
    dining_tables (id) {
        id -> Text,
        zone -> Text,
        name -> Text,
        seats -> Integer,
        active -> Integer,
        sort -> BigInt,
    }
}
diesel::table! {
    kitchen_tickets (number) {
        number -> BigInt,
        id -> Text,
        cart_id -> Text,
        created_at -> BigInt,
        bumped_at -> Nullable<BigInt>,
        ticket_json -> Text,
    }
}
diesel::table! {
    credit_notes (number) {
        number -> BigInt,
        id -> Text,
        sale_id -> Text,
        source_id -> Text,
        created_at -> BigInt,
        note_json -> Text,
    }
}
diesel::allow_tables_to_appear_in_same_query!(
    credit_notes,
    products,
    sales,
    held_carts,
    sale_voids,
    shifts,
    cash_movements,
    stock_movements,
    sale_refunds,
    staff,
    app_settings,
    tax_invoices,
    roles,
    dining_tables,
    kitchen_tickets
);

diesel::table! {
    activity_events (number) {
        number -> BigInt,
        created_at -> BigInt,
        kind -> Text,
        entity_id -> Text,
        event_json -> Text,
    }
}
