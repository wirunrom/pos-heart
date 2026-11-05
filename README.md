# POS Heart

POS Heart is a desktop point-of-sale application for retail shops and counter-service restaurants. Built with Rust and egui, it stores its primary data in a local SQLite database and supports Thai and English, mouse, keyboard, and touch input.

**Status: under development and not ready to replace a production POS.** Physical hardware, multi-device operation, and tax documents need further verification. See the [validation record](docs/validation.md) for evidence and outstanding checks.

![POS Heart sales screen with fictional data](screenshots/2026-10-14/soft/01-main/sell.png)

*This screenshot uses fictional data. See the [screenshot catalog](screenshots/2026-10-14/README.md) for more views and capture details.*

## Features

- **Point of sale:** products and options, discounts and promotions, held bills, split bills, and cash, transfer, card, or split payments.
- **Restaurant operations:** table floor, kitchen orders and tracking, and printable check bills.
- **Catalog and stock:** categories, product images, CSV import, stock movement history, ingredients and recipes, purchase orders, and receiving.
- **Sales management:** receipt history, partial refunds and full voids, shifts and cash drawers, reports, and CSV export.
- **Shop settings:** staff permissions, VAT, tax invoices and credit notes, receipt printers, and backups.
- **Devices and cloud:** optional shop LAN devices and a cloud service. See [multi-device](docs/multi-device.md) for their status and limitations.

Transfers and card payments are recorded after manual confirmation. The app does not automatically verify a transfer, charge a card, or send a refund. Tax document layouts have not been legally reviewed.

## Getting started

Install Rust stable **1.95 or later** and the native build tools for your platform: Xcode Command Line Tools on macOS or Visual Studio C++ Build Tools on Windows. The first build downloads dependencies. The standalone desktop app does not require a database server.

```sh
# Open with fictional sample data in a separate demo folder
cargo run -p pos-desktop --bin pos-heart -- --demo

# Open normally, or choose a data directory
cargo run -p pos-desktop --bin pos-heart
cargo run -p pos-desktop --bin pos-heart -- --data-dir ./data
```

On first launch, set a shop name and add products, or choose to set up later. Open a shift before taking payment. Normal data lives in the operating system's application-data directory; its location is shown in Settings → Data & backup. See the [release guide](docs/release.md) for installation and packaging. Installers are not yet fully signed.

## Project structure

| Directory | Purpose |
| --- | --- |
| [`apps/pos-desktop`](apps/pos-desktop) | Desktop interface and background work |
| [`crates/pos-domain`](crates/pos-domain) | Business rules, pricing, and sales data types |
| [`crates/pos-core`](crates/pos-core) | Database, transactions, and persistence |
| [`crates/pos-service`](crates/pos-service) | Command protocol and dispatcher |
| [`crates/pos-hub`](crates/pos-hub) | Shop LAN device connectivity |
| [`apps/pos-cloud`](apps/pos-cloud) | Optional cloud service and dashboard |
| [`design`](design) | Design system, tokens, and visual references |

The standalone app keeps data locally by default. LAN devices and cloud features require additional setup; see [multi-device](docs/multi-device.md) and the developer commands in [CLAUDE.md](CLAUDE.md).

## Checks

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Some cloud tests require PostgreSQL and `POS_CLOUD_TEST_DATABASE_URL`; see the [developer commands](CLAUDE.md#commands). A successful build or a screenshot alone does not establish runtime behavior on every platform or device.

## Documentation

- [Detailed user guide](docs/user-guide.md) — includes development-era notes; some sections describe earlier milestones.
- [Product scope](PRODUCT.md) · [Architecture](docs/architecture.md) · [Design system](design/README.md)
- [Multi-device and cloud](docs/multi-device.md) · [Installation and release](docs/release.md) · [Validation record](docs/validation.md)
- [Repository guidance](AGENTS.md) · [Developer commands and context](CLAUDE.md)

## Data and licensing

Use fictional data for development and screenshots. Do not commit shop databases, backups, credentials, or device-specific settings. The local database is not encrypted, and staff PINs do not protect the database file from direct access.

No license has been selected for the project source code. Bundled fonts retain their own licenses in [`assets/fonts`](assets/fonts).
