# Environment Configuration Guide

This guide details how to configure the **LLM over DNS** server using environment variables and `.env` files.

---

## ⚙️ Configuration Methods & Precedence

The server supports four ways to load configurations, resolved in order of priority:

1. **Environment Variables** (Highest precedence)
2. **`.env.local` File** (Local-only gitignored overrides)
3. **`.env` File** (Shared environment defaults)
4. **Hard-coded Defaults** (Lowest precedence)

---

## 🔑 LLM Gateways & API Configuration

The server natively integrates with two major LLM API gateway providers: **AnyRouter** and **OpenRouter**. 

> [!TIP]
> **AnyRouter is the highly recommended default** because it is built for high speed, low latency, native OpenAI/Anthropic compatibility, and resilient model fallback lists.

### 1. AnyRouter (Recommended)

To configure the server to route requests through the AnyRouter gateway, simply set `ANYROUTER_API_KEY`. If this key is present, the server automatically switches the base completion endpoint and defaults to optimized AnyRouter models.

* **`ANYROUTER_API_KEY`** (Required for AnyRouter)
  * **Description**: Your AnyRouter API key.
  * **Format**: Starts with `sk-ar-v1-...`
  * **Obtain**: Sign up at [anyrouter.dev](https://anyrouter.dev).
  * **Example**:
    ```env
    ANYROUTER_API_KEY=sk-ar-v1-yourkeyhere
    ```

* **`ANYROUTER_MODEL`** (Optional)
  * **Description**: A comma-separated list of LLM models to try in sequence if a model fails.
  * **Default**: `google/gemini-2.5-flash-lite,meta/llama-3.2-3b-instruct`
  * **Example**:
    ```env
    ANYROUTER_MODEL=google/gemini-2.5-flash-lite,meta/llama-3.2-3b-instruct
    ```

---

### 2. OpenRouter

If `ANYROUTER_API_KEY` is not set, the server falls back to using OpenRouter.

* **`OPENROUTER_API_KEY`** (Required if using OpenRouter)
  * **Description**: Your OpenRouter API key.
  * **Format**: Starts with `sk-or-v1-...`
  * **Obtain**: Sign up at [openrouter.ai](https://openrouter.ai).
  * **Example**:
    ```env
    OPENROUTER_API_KEY=sk-or-v1-yourkeyhere
    ```

* **`OPENROUTER_MODEL`** (Optional)
  * **Description**: Comma-separated list of fallback models for OpenRouter.
  * **Default**: `nvidia/nemotron-nano-9b-v2:free,meituan/longcat-flash-chat:free,minimax/minimax-m2:free`
  * **Example**:
    ```env
    OPENROUTER_MODEL=nvidia/nemotron-nano-9b-v2:free
    ```

---

## 🌐 DNS Server Configuration

* **`DNS_PORT`** or **`PORT`** (Optional)
  * **Description**: The port the DNS server will bind to. (`PORT` takes precedence over `DNS_PORT` if both are set).
  * **Default**: `53` (Requires standard root privileges on Linux/macOS).
  * **Alternative**: `5454` or `5353` for local non-root development.
  * **Example**:
    ```env
    DNS_PORT=5454
    ```

* **`DNS_ADDRESS`** or **`HOST`** (Optional)
  * **Description**: The IP address to bind the DNS server listener to. (`HOST` takes precedence).
  * **Default**: `0.0.0.0` (listens on all network interfaces).
  * **Alternative**: `127.0.0.1` (localhost only).
  * **Example**:
    ```env
    DNS_ADDRESS=127.0.0.1
    ```

---

## 📝 Logging Configuration

* **`RUST_LOG`** (Optional)
  * **Description**: Control the logging verbosity.
  * **Default**: `info`
  * **Available Levels**: `trace`, `debug`, `info`, `warn`, `error`
  * **Example**:
    ```env
    RUST_LOG=info,llm_over_dns=debug
    ```

---

## 📋 Comprehensive Reference Table

| Variable | Precedence Fallback | Default Value | Description |
|---|---|---|---|
| `ANYROUTER_API_KEY` | None | None | Primary AnyRouter API key. Set this to activate AnyRouter. |
| `OPENROUTER_API_KEY` | None | None | OpenRouter API key. Used if `ANYROUTER_API_KEY` is empty. |
| `ANYROUTER_MODEL` | None | `google/gemini-2.5-flash-lite,meta/llama-3.2-3b-instruct` | List of models used in fallback order for AnyRouter. |
| `OPENROUTER_MODEL` | None | `nvidia/nemotron-nano-9b-v2:free,meituan/longcat-flash-chat:free...` | List of fallback models for OpenRouter. |
| `DNS_PORT` | `PORT` | `53` | Listening port for UDP DNS server. |
| `DNS_ADDRESS` | `HOST` | `0.0.0.0` | IP binding address. |
| `RUST_LOG` | None | `info` | Logging verbosity filter. |

---

## 🔒 Security Best Practices

1. **Protect your `.env`**: Make sure `.env` and `.env.local` are explicitly added to `.gitignore`. Never commit keys to a shared VCS.
2. **Restrict Permissions**: On Linux systems, keep your local `.env` file secure:
   ```bash
   chmod 600 .env
   ```
3. **API Key Safety**: Rotate keys regularly via the gateways' web dashboards.
