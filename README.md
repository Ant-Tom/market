# GhostMarket Escrow

Decentralized P2P marketplace escrow on Solana. Anchor program for safe USDC trades between strangers.

## Что делает контракт

```
buyer ── pays USDC ──► [escrow vault PDA] ◄── PDA authority
                              │
                              ├─ buyer confirms ─► seller gets USDC, treasury gets fee
                              ├─ 14d timeout    ─► buyer gets full refund
                              └─ both agree     ─► cancel before ship, full refund
```

- Деньги покупателя замораживаются в `vault` — token account, authority которого сам PDA эскроу.
- Никто кроме программы не может тронуть vault. Даже админ.
- 4 финальных состояния: `Completed`, `Refunded` (timeout), `Cancelled`, и одно промежуточное `Shipped`.

## Структура

```
ghostmarket-escrow/
├── Anchor.toml
├── Cargo.toml
├── package.json
├── tsconfig.json
├── programs/ghostmarket-escrow/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                  # точка входа, declare_id!, #[program]
│       ├── constants.rs            # seeds, лимиты, default-значения
│       ├── errors.rs               # все custom errors
│       ├── state.rs                # Config, Escrow, EscrowStatus, events
│       └── instructions/
│           ├── mod.rs
│           ├── initialize_config.rs
│           ├── update_config.rs
│           ├── create_escrow.rs    # buyer pays, vault создаётся
│           ├── mark_shipped.rs     # seller фиксирует tracking_hash
│           ├── confirm_received.rs # buyer ОК → seller paid + fee → treasury
│           ├── claim_timeout.rs    # после timeout → buyer refund
│           └── cancel_before_ship.rs # buyer/seller отмена до отправки
└── tests/
    └── ghostmarket-escrow.ts       # 8 тестов: happy path + все ошибки
```

## Установка и сборка

### Зависимости

- **Rust 1.79+** (более старые не подойдут — современная Solana-зависимость `cmov 0.5.3` требует фичу `edition2024`, стабилизированную только в 1.79). Лучший способ — `rustup` (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`); системный `apt install rustc` обычно слишком старый.
- Solana CLI 1.18+ (`sh -c "$(curl -sSfL https://release.solana.com/v1.18.20/install)"`)
- Anchor 0.30.1 (`cargo install --git https://github.com/coral-xyz/anchor avm --locked && avm install 0.30.1 && avm use 0.30.1`)
- Node.js 18+ и yarn

### Локальная сборка и тесты

```bash
cd ghostmarket-escrow
yarn install

solana-keygen new -o ~/.config/solana/id.json

anchor build

anchor keys sync

anchor test
```

`anchor test` поднимает локальный валидатор, деплоит программу, прогоняет 8 тестов:

1. `initializes config` — создание Config PDA с feeBps=800, timeout=14d
2. `happy path` — полный цикл: pay → ship → confirm → seller получил, treasury получил комиссию
3. `rejects self-purchase` — buyer == seller отклоняется
4. `rejects mark_shipped from non-seller` — атакующий не может отметить отправку
5. `rejects confirm_received from non-buyer` — атакующий не может подтвердить получение
6. `buyer can cancel before ship` — отмена до отправки → полный возврат
7. `cannot cancel after seller ships` — после shipped отмена невозможна
8. `admin can pause/unpause` + `non-admin cannot update config`

### Деплой на devnet

```bash
solana config set --url devnet
solana airdrop 2

anchor build
anchor deploy --provider.cluster devnet

# инициализация (один раз)
# нужен mint USDC на devnet: EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v
ts-node migrations/initialize.ts
```

## Параметры по умолчанию

| Параметр | Значение | Где менять |
|---|---|---|
| Комиссия | 800 bps = 8% | `update_config(fee_bps=...)` |
| Timeout | 14 дней | `update_config(timeout_seconds=...)` |
| Max fee | 1500 bps = 15% | константа в `constants.rs` |
| Min/max timeout | 1d / 60d | константа в `constants.rs` |
| Token | USDC (или любой SPL mint) | `initialize_config` |

## Что дальше

- [ ] Индексер на Helius webhooks → Postgres → GraphQL
- [ ] TS SDK (`@ghostmarket/sdk`) с хелперами для фронта
- [ ] Подключить готовый React-фронт к реальной программе
- [ ] IPFS-загрузка через web3.storage
- [ ] Споры (Phase 2): бондированные арбитры
- [ ] $GHOST токен и стейкинг (Phase 2)

## Безопасность — известные ограничения v1

- Нет механизма споров: только timeout или согласие сторон. Это by design — споры идут в v2.
- Админ может приостановить новые эскроу (`pause`), но не может тронуть уже залоченные средства.
- `tracking_hash` — это `sha256(track_number)`, реальный трек хранится off-chain и шарится через чат.
- Контракт хранит `payment_mint` в Config — все эскроу идут через один токен (USDC). Мульти-токен — в v2.
- `cancel_before_ship` требует две подписи (signer + buyer_signer_payer): если seller инициирует cancel, buyer должен тоже подписать. Это защищает от ситуации когда seller форсит отмену без согласия buyer-а. Если buyer инициирует — он подписывает оба раза одним keypair.

## Что я НЕ проверил перед коммитом — будь готов

Код был ревью-нут построчно, но **полная сборка не запущена** в моём окружении (Rust 1.75, нужен 1.79+). На Mac mini первая `anchor build` может найти:

- опечатки в типах после правок,
- расхождения между API anchor-spl 0.30.1 которое я помню и реальностью,
- minor warnings которые надо подавить.

Если что-то не сходится — пришли вывод `anchor build`, поправим.
