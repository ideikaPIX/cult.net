# 🤖 MASTER PROMPT: CULT.NET — Децентрализованный CLI Мессенджер

## 📋 Общее описание

**CULT.NET** — это минималистичный, децентрализованный P2P CLI мессенджер на Rust с end-to-end шифрованием RSA-2048. Архитектура: клиенты подключаются к VPS relay-серверу, который пропускает только зашифрованные сообщения и никогда не хранит историю.

**Язык:** Rust  
**Крипто:** RSA-2048 (простая встроенная библиотека)  
**Хранилище ключей:** JSON в `~/.cult/authinfo/`  
**Сеть:** WebSocket (Relay модель)  
**UI:** CLI (ультраминимализм)

---

## 🔐 Система аккаунтов и регистрации

### Структура аккаунта

Каждый аккаунт состоит из:
- **username** — любое имя, может быть повторяющимся (e.g., `platon`)
- **peer_id** — последние 4 символа хеша публичного ключа (e.g., `B84b`)
- **full_address** — `username#peer_id@cult.net` (e.g., `platon#B84b@cult.net`)
- **public_key** — RSA публичный ключ (отправляется на VPS)
- **private_key** — RSA приватный ключ (НИКОГДА не отправляется, хранится локально в `~/.cult/authinfo/accounts.json` с правами `0600`)

### Процесс регистрации (полностью автоматизирован, скрыт от пользователя)

**Пользователь видит только:**
```
cult> login
Enter username: platon
⏳ Registering...
✓ Welcome, platon#B84b@cult.net!
```

**За кулисами происходит:**

#### ВАРИАНТ A: ОНЛАЙН регистрация (предпочтительно)
```
1. Клиент подключен к VPS (WebSocket)
2. Генерирует RSA-2048 пару ключей локально
3. Отправляет на VPS:
   {
     "action": "register",
     "username": "platon",
     "public_key": "-----BEGIN PUBLIC KEY-----\n...\n-----END PUBLIC KEY-----"
   }
4. VPS генерирует peer_id = hash(public_key)[-4:]
5. VPS отвечает:
   {
     "status": "ok",
     "peer_id": "B84b",
     "full_address": "platon#B84b@cult.net"
   }
6. Клиент сохраняет в ~/.cult/authinfo/accounts.json:
   {
     "accounts": [
       {
         "username": "platon",
         "peer_id": "B84b",
         "full_address": "platon#B84b@cult.net",
         "public_key": "...",
         "private_key": "...",
         "created_at": "2026-05-15T10:30:00Z",
         "is_active": true,
         "confirmed_online": true,
         "vps_confirmed": true
       }
     ]
   }
7. is_active устанавливается в true
8. CLI запускается с полным функционалом
```

#### ВАРИАНТ B: ОФФЛАЙН регистрация (если нет интернета)
```
1. Клиент НЕ может подключиться к VPS
2. Генерирует RSA-2048 пару ключей локально
3. Генерирует peer_id локально: peer_id = hash(public_key)[-4:]
4. Сохраняет в ~/.cult/authinfo/accounts.json с:
   - is_active: false
   - confirmed_online: false
   - vps_confirmed: false
5. Выводит сообщение:
   ⚠️  Be online to confirm your user
   Registration saved locally.
6. CLI работает в offline mode (нет чатов, только просмотр истории)

Когда пользователь подключается онлайн:
1. Запускает: cult connect
2. Клиент автоматически отправляет все неподтвержденные аккаунты на VPS
3. VPS подтверждает peer_id (или генерирует новый, если конфликт)
4. is_active меняется на true
5. Полный функционал становится доступен
```

---

## 🔑 Криптография

### Генерация ключей

- **Алгоритм:** RSA-2048 (встроенная библиотека `rsa` crate)
- **Формат:** PKCS#1 v1.5 (для простоты)
- **Размер:** 2048 бит
- **Генерация:** При каждом `login` (ново-новый для каждого аккаунта)

### Шифрование сообщений

```rust
plaintext = "привет!"
public_key_recipient = load_from_contacts()
ciphertext = encrypt(plaintext, public_key_recipient)
send_to_vps(ciphertext)

// На приемной стороне:
private_key_self = load_from_accounts.json()
plaintext = decrypt(ciphertext, private_key_self)
display_in_chat(plaintext)
```

### Безопасность

- ✅ Приватный ключ НИКОГДА не отправляется на сервер
- ✅ Приватный ключ НИКОГДА не логируется
- ✅ Файл `accounts.json` имеет права доступа `0600` (только владелец)
- ✅ При передаче по сети — только зашифрованное содержимое
- ✅ VPS не может дешифровать сообщения

---

## 📡 Архитектура сервера (VPS Relay)

### Функции VPS

1. **Регистрация аккаунтов:**
   - Принимает `{username, public_key}`
   - Генерирует `peer_id` = `hash(public_key)[-4:]`
   - Сохраняет в памяти (или легкой БД) маппинг: `username#peer_id → public_key`
   - Возвращает `{peer_id, full_address}`

2. **Получение публичного ключа:**
   - Запрос: `{action: "get_public_key", target: "username#peer_id@cult.net"}`
   - Ответ: `{public_key, online_status}`

3. **Relay сообщений:**
   - Принимает зашифрованное сообщение от User1
   - Проверяет авторизацию User1
   - Если User2 онлайн → сразу отправляет
   - Если User2 офлайн → кладет в queue с TTL 24h
   - СРАЗУ удаляет из памяти (не хранит)

4. **Online/Offline статусы:**
   - Отслеживает WebSocket соединения
   - Отправляет статус update всем пирам: `{user: "...", status: "online/offline"}`

### Что VPS НЕ делает

- ❌ Не хранит историю сообщений
- ❌ Не видит содержание сообщений (всё зашифровано)
- ❌ Не управляет приватными ключами
- ❌ Не ведет логи сообщений (только служебные логи)

---

## 🖥️ CLI интерфейс (ультраминимализм)

### Главный экран

```
cult.net | platon#B84b | connected
[p]eers  [c]hat  [a]dd  [s]witch  [q]uit

>
```

### Команда `peers` — Список контактов

```
> p

platon#B84b       ✓
platon#2F3a       ⏳
alex#7C9d         ✗
admin#F92k        ✓

[p <num>] peer info | [c <num>] chat | [b]ack

>
```

**Статусы:**
- `✓` — confirmed online (добавлен, подтвержден VPS, онлайн)
- `⏳` — pending online (добавлен, ожидает подтверждения)
- `✗` — offline (последний контакт > 5 минут назад)

### Команда `chat <peer>` — Открыть чат

```
> c alex#7C9d

alex#7C9d | ✗ | 2.5s
─
14:23 alex: привет!
14:25 you: привет!
14:26 alex: как дела?
14:28 you: всё (pending)
─
[↑] scroll | [ctrl+c] exit

>
```

**Нижний статус бара (одна строка):**
```
alex#7C9d | ✗ | 2.5s
  └─peer   └─status └─delay последнего контакта
```

**Статусы в чате:**
- `✓` — online
- `✗` — offline
- `⏳` — pending

**Статусы сообщений:**
- `(pending)` — отправлено на VPS, ждет доставки
- `(delivered)` — доставлено (если включено подтверждение)
- `(read)` — прочитано (опционально)

### Команда `add <peer>` — Добавить контакт

```
> a platon#2F3a@cult.net

Looking up platon#2F3a@cult.net...
✓ Found | platon | 2F3a | online

✓ platon#2F3a added

>
```

**За кулисами:**
1. Клиент запрашивает публичный ключ у VPS
2. VPS отвечает публичным ключом + статусом
3. Контакт сохраняется локально в `~/.cult/chats/contacts.json`

### Команда `switch` — Переключение аккаунтов

```
> s

platon#B84b       ✓ active
platon#2F3a       ⏳
alex#7C9d         ✗

[select number]

>
```

После выбора:
```
Switched to platon#2F3a@cult.net

>
```

### Стартовый экран (онлайн)

```
🔐 cult.net
✓ connected | platon#B84b | 4 peers | 2 unread

[enter]
```

### Стартовый экран (оффлайн)

```
🔐 cult.net
✗ offline mode
⚠️ platon#B84b (unconfirmed)

Limited functionality

[enter]
```

### Быстрая помощь (при `?`)

```
> ?

[p]eers        view your contacts
[c]hat <name>  open chat
[a]dd <peer>   add contact
[s]witch       change account
[q]uit         exit

[?] help | [enter] back

>
```

---

## 📂 Структура файлов

```
~/.cult/
├── authinfo/
│   └── accounts.json          # ПРИВАТНО! (chmod 0600)
│                              # {username, peer_id, keys, status}
├── chats/
│   ├── contacts.json          # {peer → public_key}
│   └── alex#7C9d.db           # SQLite история с этим peer
├── config.json                # настройки (VPS адрес, параметры)
└── logs/
    └── cult.log               # логи приложения
```

### accounts.json (ПРИВАТНЫЙ)

```json
{
  "accounts": [
    {
      "username": "platon",
      "peer_id": "B84b",
      "full_address": "platon#B84b@cult.net",
      "public_key": "-----BEGIN PUBLIC KEY-----\n...\n-----END PUBLIC KEY-----",
      "private_key": "-----BEGIN RSA PRIVATE KEY-----\n...\n-----END RSA PRIVATE KEY-----",
      "created_at": "2026-05-15T10:30:00Z",
      "is_active": true,
      "confirmed_online": true,
      "vps_confirmed": true
    }
  ]
}
```

### contacts.json

```json
{
  "contacts": [
    {
      "username": "platon",
      "peer_id": "2F3a",
      "full_address": "platon#2F3a@cult.net",
      "public_key": "-----BEGIN PUBLIC KEY-----\n...\n-----END PUBLIC KEY-----",
      "added_at": "2026-05-15T11:00:00Z",
      "last_message": "2026-05-15T14:28:00Z"
    }
  ]
}
```

### Формат БД (SQLite для каждого чата)

```
Table: messages
  id          INTEGER PRIMARY KEY
  timestamp   TEXT (ISO 8601)
  sender      TEXT (peer#id@cult.net)
  content     TEXT (plaintext, расшифрован на клиенте)
  status      TEXT (delivered/pending/read)
  is_yours    BOOLEAN
```

---

## 🔄 Полный цикл сообщения

### Пользователь 1 отправляет сообщение Пользователю 2

```
1. User1 вводит в чате:
   > привет, alex!

2. Клиент User1:
   ├─ Загружает public_key alex из contacts.json
   ├─ Шифрует: ciphertext = encrypt("привет, alex!", public_key_alex)
   ├─ Отправляет на VPS:
   │  {
   │    "action": "send_message",
   │    "from": "platon#B84b@cult.net",
   │    "to": "alex#7C9d@cult.net",
   │    "encrypted_content": "base64_encrypted_blob",
   │    "timestamp": "2026-05-15T14:28:00Z"
   │  }
   ├─ Сохраняет локально с статусом (pending)
   └─ Выводит в чате: [14:28] you: привет, alex! (pending)

3. VPS:
   ├─ Проверяет авторизацию User1 (по WebSocket)
   ├─ Проверяет формат `to`
   ├─ Если alex#7C9d онлайн:
   │  ├─ Отправляет сообщение в WebSocket alex#7C9d
   │  ├─ Ждет подтверждения (delivery receipt)
   │  └─ Удаляет из памяти
   └─ Если alex#7C9d офлайн:
      ├─ Кладет в queue с TTL 24h
      └─ При подключении alex отправляет

4. User2 получает:
   ├─ Зашифрованное сообщение:
   │  {
   │    "from": "platon#B84b@cult.net",
   │    "encrypted_content": "base64_encrypted_blob"
   │  }
   ├─ Загружает private_key из accounts.json (активный аккаунт)
   ├─ Дешифрует: plaintext = decrypt(encrypted_content, private_key)
   ├─ Отображает в чате: [14:28] platon#B84b: привет, alex!
   ├─ Сохраняет расшифрованное в БД
   └─ Отправляет delivery receipt на VPS

5. VPS отправляет User1:
   ├─ Delivery receipt: {delivered: true}
   └─ User1 обновляет статус: [14:28] you: привет, alex! ✓
```

---

## 🛠️ Технический стек

### Основные крейты

**Клиент:**
```toml
tokio = "1"                    # Асинхронный runtime
tokio-tungstenite = "0.23"     # WebSocket клиент
rsa = "0.9"                    # RSA шифрование
serde_json = "1"               # JSON парсинг
serde = { version = "1", features = ["derive"] }
rusqlite = { version = "0.32", features = ["bundled"] }  # SQLite
crossterm = "0.28"             # TUI (клавиатура, движение курсора)
chrono = "0.4"                 # Временные метки
sha2 = "0.10"                  # Хеширование
```

**Сервер:**
```toml
tokio = "1"
tokio-tungstenite = "0.23"
serde_json = "1"
serde = { version = "1", features = ["derive"] }
uuid = { version = "1", features = ["v4", "serde"] }
chrono = "0.4"
tracing = "0.1"
tracing-subscriber = "0.3"
sha2 = "0.10"
```

### Структура проекта

```
cult-net/
├── Cargo.toml
├── Cargo.lock
├── src/
│   ├── main.rs
│   ├── lib.rs
│   │
│   ├── client/
│   │   ├── mod.rs
│   │   ├── cli.rs              # CLI интерфейс, обработка команд
│   │   ├── crypto.rs           # RSA генерация, encrypt/decrypt
│   │   ├── network.rs          # WebSocket подключение
│   │   ├── storage.rs          # authinfo/contacts/chat БД
│   │   ├── auth.rs             # Управление аккаунтами
│   │   └── messages.rs         # Логика обработки сообщений
│   │
│   └── server/
│       ├── mod.rs
│       ├── main.rs             # Server entry point
│       ├── network.rs          # WebSocket сервер (axum)
│       ├── relay.rs            # Relay логика
│       ├── registry.rs         # Хранилище публичных ключей
│       ├── queue.rs            # Очередь для оффлайн
│       └── auth.rs             # Проверка авторизации
├── tests/
├── README.md
└── .gitignore
```

---

## 🚀 Этапы реализации (приоритет)

1. **Крипто модуль** (`client/crypto.rs`)
   - Генерация RSA-2048 пары ключей
   - Шифрование/дешифрование
   - Сохранение в PEM формате

2. **Управление аккаунтами** (`client/auth.rs`, `client/storage.rs`)
   - Создание аккаунта (login)
   - Сохранение в `accounts.json` (chmod 0600)
   - Загрузка приватного ключа
   - Переключение между аккаунтами

3. **Сетевой модуль сервера** (`server/network.rs`)
   - WebSocket сервер (Tokio + Tungstenite)
   - Обработка подключений/отключений
   - Парсинг входящих сообщений

4. **Регистрация и relay** (`server/relay.rs`, `server/registry.rs`)
   - Регистрация новых пользователей
   - Хранилище публичных ключей
   - Relay сообщений между клиентами

5. **CLI интерфейс клиента** (`client/cli.rs`)
   - Главное меню
   - Команды: peers, chat, add, switch, quit
   - Ввод сообщений и вывод истории

6. **WebSocket клиент** (`client/network.rs`)
   - Подключение к VPS
   - Отправка/получение сообщений
   - Статус синхронизация

7. **Локальное хранилище** (`client/storage.rs`)
   - SQLite для истории чатов
   - Загрузка/сохранение контактов
   - Управление БД

8. **Оффлайн режим** (`client/auth.rs`, `server/queue.rs`)
   - Регистрация без интернета
   - Сохранение неподтвержденных аккаунтов
   - Очередь сообщений на сервере

9. **Интеграция и тестирование**
   - E2E тесты
   - Обработка ошибок
   - Optimize производительность

---

## 🎯 Ключевые ограничения

| Функция | Статус | Примечание |
|---------|--------|-----------|
| Оффлайн регистрация | ✓ Да | Локально, требует онлайн для подтверждения |
| Оффлайн сообщения | ✗ Нет | Сообщения отправляются только онлайн |
| История на VPS | ✗ Нет | Удаляется после доставки или TTL 24h |
| E2E шифрование | ✓ RSA-2048 | Полностью end-to-end |
| Множество аккаунтов | ✓ Да | Без ограничений |
| Повторяющиеся username | ✓ Да | Различаются по peer_id |
| Анонимность | ⚠️ Частичная | VPS видит metadata (кто с кем) |
| Передача файлов | ✗ Нет | Только текстовые сообщения |
| Групповые чаты | ✗ Нет | Только P2P |

---

## 📝 Дополнительные требования

### Безопасность

- ✅ Приватные ключи НИКОГДА не отправляются в сеть
- ✅ Приватные ключи хранятся с правами `0600`
- ✅ Никаких логов с содержимым сообщений (только служебные)
- ✅ Валидация всех входных данных (username, peer_id)
- ✅ Защита от replay-атак (timestamp + nonce)

### Производительность

- ⚡ Субсекундное отображение сообщений
- ⚡ Быстрая загрузка истории (1000+ сообщений)
- ⚡ Минимальное потребление памяти
- ⚡ Оптимизация для слабых сетей

### Пользовательский опыт

- 🎯 Полная автоматизация регистрации
- 🎯 Минималистичный, интуитивный интерфейс
- 🎯 Четкие сообщения об ошибках
- 🎯 Offline-first подход

---

**Этот промпт содержит всю информацию для разработки CULT.NET. Готово к использованию! 🚀**
