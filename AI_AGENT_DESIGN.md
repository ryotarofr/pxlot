# AI Agent 基本設計 — LLM によるピクセルアート自律生成

## 概要

チャット形式で LLM と会話し、LLM がエディタの描画ツールを自律的に操作してピクセルアートを作成する機能。
Claude API の **Tool Use (Function Calling) + Vision** を活用し、ブラウザ上で完結するエージェントループを実現する。

---

## 実現可能性の評価

### 結論: **実現可能**

### 根拠

| 要素 | 状況 | 評価 |
|---|---|---|
| 描画 API | `pencil_pixel`, `draw_line`, `draw_rect`, `flood_fill` 等が関数として独立 | ◎ LLM ツールに直接マッピング可能 |
| キャンバス操作 | `add_layer`, `move_layer`, Canvas::new 等が明確な API | ◎ そのまま公開可能 |
| 状態取得 | `flatten_frame()` で PNG 化、`eyedropper()` で色取得 | ◎ Vision で現状確認可能 |
| LLM API | Claude API は Tool Use + Vision をサポート | ◎ エージェントループに最適 |
| 実行環境 | WASM (ブラウザ) から HTTPS で API 呼び出し可能 | ○ CORS 対応必要 |
| 描画品質 | ピクセルアートは低解像度・離散的 → LLM が座標指定しやすい | ◎ 32x32 なら 1024 ピクセル |

### リスク・課題

| 課題 | 影響度 | 対策 |
|---|---|---|
| API キー管理 | 中 | ユーザーが自分のキーを入力、localStorage に保存 |
| API コスト | 中 | トークン使用量の表示、1 回の生成に上限設定 |
| レイテンシ | 中 | ストリーミング表示、ステップごとにキャンバス更新 |
| 描画精度 | 中 | Vision フィードバックループで自己修正 |
| CORS | 低 | Claude API は CORS 対応済み（ブラウザから直接呼べる） |

---

## アーキテクチャ

```
┌─────────────────────────────────────────────────────┐
│  ブラウザ (WASM)                                      │
│                                                       │
│  ┌──────────┐    ┌──────────────┐    ┌─────────────┐ │
│  │ Chat UI  │◄──►│ Agent Runner │◄──►│ Tool Bridge │ │
│  │          │    │              │    │             │ │
│  │ メッセージ │    │ ループ制御    │    │ Canvas操作  │ │
│  │ 表示      │    │ ツール実行    │    │ 状態取得    │ │
│  └──────────┘    └──────┬───────┘    └─────────────┘ │
│                         │                             │
│                         ▼                             │
│                  ┌──────────────┐                     │
│                  │ Claude API   │                     │
│                  │ (HTTPS)      │                     │
│                  └──────────────┘                     │
└─────────────────────────────────────────────────────┘
```

### コンポーネント構成

```
app/src/
├── components/
│   └── ai_chat.rs          # チャット UI コンポーネント
├── ai/
│   ├── mod.rs              # モジュール定義
│   ├── agent.rs            # エージェントループ制御
│   ├── api_client.rs       # Claude API 呼び出し (fetch)
│   ├── tools.rs            # LLM ツール定義 & 実行ブリッジ
│   └── message.rs          # メッセージ型定義
```

---

## エージェントループ

```
User: 「剣のアイコンを描いて」
         │
         ▼
┌─ Agent Loop ──────────────────────────────────┐
│                                                │
│  1. システムプロンプト + ユーザーメッセージ      │
│     + キャンバス画像 (base64 PNG) を送信        │
│                                                │
│  2. LLM がツール呼び出しを返す                  │
│     例: set_pixels([{x:15,y:8,color:"#8B4513"}])│
│                                                │
│  3. ツールを Canvas に対して実行                 │
│     → キャンバスをリアルタイム更新・再描画       │
│                                                │
│  4. ツール実行結果を LLM に返す                  │
│     + 更新後のキャンバス画像                     │
│                                                │
│  5. LLM が次のアクションを決定                   │
│     → ツール呼び出し → 3 に戻る                 │
│     → テキスト応答 → 完了                       │
│                                                │
│  ※ 最大ループ回数で強制終了 (安全弁)            │
└────────────────────────────────────────────────┘
```

### ループ制御パラメータ

```rust
struct AgentConfig {
    max_turns: usize,        // 最大ターン数 (デフォルト: 20)
    max_tokens: usize,       // 1 応答あたりの最大トークン (4096)
    send_image_every: usize, // N ターンごとに画像を送信 (デフォルト: 3)
    model: String,           // "claude-sonnet-4-6" 等
}
```

- **画像送信の頻度**: 毎ターン送ると高コストなので、N ターンごと or ツール `get_canvas_image` を LLM が明示的に呼んだときのみ
- **強制停止**: `max_turns` 超過、またはユーザーが「停止」ボタンを押下

---

## LLM に公開するツール定義

### 描画ツール

```json
[
  {
    "name": "set_pixels",
    "description": "Set one or more pixels on the active layer. Use this for detailed pixel-by-pixel drawing. Coordinates are relative to the frame (0,0 is top-left of the canvas).",
    "input_schema": {
      "type": "object",
      "properties": {
        "pixels": {
          "type": "array",
          "items": {
            "type": "object",
            "properties": {
              "x": { "type": "integer" },
              "y": { "type": "integer" },
              "color": { "type": "string", "description": "#RRGGBB hex color" }
            },
            "required": ["x", "y", "color"]
          },
          "description": "Array of pixels to set"
        }
      },
      "required": ["pixels"]
    }
  },
  {
    "name": "draw_line",
    "description": "Draw a straight line between two points.",
    "input_schema": {
      "type": "object",
      "properties": {
        "x0": { "type": "integer" },
        "y0": { "type": "integer" },
        "x1": { "type": "integer" },
        "y1": { "type": "integer" },
        "color": { "type": "string" }
      },
      "required": ["x0", "y0", "x1", "y1", "color"]
    }
  },
  {
    "name": "draw_rect",
    "description": "Draw a rectangle outline.",
    "input_schema": {
      "type": "object",
      "properties": {
        "x0": { "type": "integer" },
        "y0": { "type": "integer" },
        "x1": { "type": "integer" },
        "y1": { "type": "integer" },
        "color": { "type": "string" },
        "filled": { "type": "boolean", "description": "If true, draw filled rectangle" }
      },
      "required": ["x0", "y0", "x1", "y1", "color"]
    }
  },
  {
    "name": "draw_ellipse",
    "description": "Draw an ellipse within the bounding box defined by two corners.",
    "input_schema": {
      "type": "object",
      "properties": {
        "x0": { "type": "integer" },
        "y0": { "type": "integer" },
        "x1": { "type": "integer" },
        "y1": { "type": "integer" },
        "color": { "type": "string" },
        "filled": { "type": "boolean" }
      },
      "required": ["x0", "y0", "x1", "y1", "color"]
    }
  },
  {
    "name": "flood_fill",
    "description": "Fill a contiguous region of the same color starting from (x, y).",
    "input_schema": {
      "type": "object",
      "properties": {
        "x": { "type": "integer" },
        "y": { "type": "integer" },
        "color": { "type": "string" }
      },
      "required": ["x", "y", "color"]
    }
  }
]
```

### キャンバス管理ツール

```json
[
  {
    "name": "get_canvas_info",
    "description": "Get current canvas dimensions, layer count, and active layer index.",
    "input_schema": {
      "type": "object",
      "properties": {}
    }
  },
  {
    "name": "get_canvas_image",
    "description": "Get the current canvas as a base64 PNG image to visually inspect your work.",
    "input_schema": {
      "type": "object",
      "properties": {}
    }
  },
  {
    "name": "clear_canvas",
    "description": "Clear all pixels on the active layer (set to transparent).",
    "input_schema": {
      "type": "object",
      "properties": {}
    }
  },
  {
    "name": "resize_canvas",
    "description": "Resize the canvas to new dimensions. Existing content is lost.",
    "input_schema": {
      "type": "object",
      "properties": {
        "width": { "type": "integer", "minimum": 8, "maximum": 128 },
        "height": { "type": "integer", "minimum": 8, "maximum": 128 }
      },
      "required": ["width", "height"]
    }
  }
]
```

### レイヤー管理ツール

```json
[
  {
    "name": "add_layer",
    "description": "Add a new transparent layer above the current one.",
    "input_schema": {
      "type": "object",
      "properties": {
        "name": { "type": "string" }
      },
      "required": ["name"]
    }
  },
  {
    "name": "select_layer",
    "description": "Select a layer by index to draw on.",
    "input_schema": {
      "type": "object",
      "properties": {
        "index": { "type": "integer" }
      },
      "required": ["index"]
    }
  }
]
```

### 応答専用ツール

```json
[
  {
    "name": "finish",
    "description": "Call this when the artwork is complete. Provide a summary of what was created.",
    "input_schema": {
      "type": "object",
      "properties": {
        "summary": { "type": "string" }
      },
      "required": ["summary"]
    }
  }
]
```

---

## ツール実行ブリッジ (Rust 側)

```rust
/// LLM ツール呼び出しを Canvas 操作に変換する
pub fn execute_tool(
    name: &str,
    input: &serde_json::Value,
    canvas: &mut Canvas,
    history: &mut History,
) -> Result<serde_json::Value, String> {
    match name {
        "set_pixels" => {
            let mut cmd = Command::new("AI: Set Pixels");
            let pixels = input["pixels"].as_array().ok_or("invalid pixels")?;
            for p in pixels {
                let x = p["x"].as_u64().ok_or("invalid x")? as u32;
                let y = p["y"].as_u64().ok_or("invalid y")? as u32;
                let color = Color::from_hex(p["color"].as_str().unwrap_or("#000000"))
                    .unwrap_or(Color::BLACK);
                // フレーム座標 → バッファ座標に変換
                let bx = canvas.to_buf_x(x as i32) as u32;
                let by = canvas.to_buf_y(y as i32) as u32;
                pencil_pixel(canvas, bx, by, color, &mut cmd);
            }
            if !cmd.is_empty() {
                history.push(cmd);
            }
            Ok(json!({"status": "ok", "pixels_set": pixels.len()}))
        }

        "draw_line" => {
            let mut cmd = Command::new("AI: Line");
            let (x0, y0, x1, y1) = parse_coords(input)?;
            let color = parse_color(input)?;
            let (bx0, by0) = to_buf(canvas, x0, y0);
            let (bx1, by1) = to_buf(canvas, x1, y1);
            draw_line(canvas, bx0, by0, bx1, by1, color, &mut cmd);
            history.push(cmd);
            Ok(json!({"status": "ok"}))
        }

        "flood_fill" => {
            let mut cmd = Command::new("AI: Fill");
            let x = input["x"].as_i64().ok_or("invalid x")? as u32;
            let y = input["y"].as_i64().ok_or("invalid y")? as u32;
            let color = parse_color(input)?;
            let (bx, by) = to_buf(canvas, x as i32, y as i32);
            flood_fill(canvas, bx as u32, by as u32, color, &mut cmd);
            history.push(cmd);
            Ok(json!({"status": "ok"}))
        }

        "get_canvas_info" => {
            Ok(json!({
                "width": canvas.frame_width(),
                "height": canvas.frame_height(),
                "layers": canvas.layers.len(),
                "active_layer": canvas.active_layer,
            }))
        }

        "get_canvas_image" => {
            // flatten → PNG encode → base64
            let png_data = png_format::export_png(canvas)?;
            let b64 = base64::encode(&png_data);
            Ok(json!({"image_base64": b64}))
        }

        "finish" => {
            let summary = input["summary"].as_str().unwrap_or("");
            Ok(json!({"status": "finished", "summary": summary}))
        }

        _ => Err(format!("Unknown tool: {}", name)),
    }
}
```

---

## Claude API 呼び出し (ブラウザ側)

### HTTP クライアント

WASM 環境では `reqwest` が使えないため、`web_sys::fetch` または `gloo-net` を使用する。

```rust
use gloo_net::http::Request;

pub async fn call_claude_api(
    api_key: &str,
    messages: &[Message],
    tools: &[ToolDefinition],
    model: &str,
) -> Result<ApiResponse, String> {
    let body = json!({
        "model": model,
        "max_tokens": 4096,
        "system": SYSTEM_PROMPT,
        "messages": messages,
        "tools": tools,
    });

    let response = Request::post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("anthropic-dangerous-direct-browser-access", "true")
        .header("content-type", "application/json")
        .body(body.to_string())
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let json: ApiResponse = response.json().await.map_err(|e| e.to_string())?;
    Ok(json)
}
```

### ヘッダーに関する注意

- `anthropic-dangerous-direct-browser-access: true` — ブラウザからの直接アクセスに必要
- API キーがクライアント側に露出するため、ユーザー自身のキーを使用する前提

---

## システムプロンプト

```text
You are a pixel art assistant integrated into the pxlot pixel art editor.
You create pixel art by calling drawing tools on the canvas.

## Rules
- The canvas uses a coordinate system where (0,0) is the top-left corner.
- Colors are specified as "#RRGGBB" hex strings.
- Work methodically: plan the drawing, then execute step by step.
- Use `get_canvas_image` to visually check your work periodically.
- Use layers strategically (e.g., background on layer 0, details on layer 1).
- For efficiency, use `set_pixels` with batched pixel arrays rather than one pixel at a time.
- Use geometric tools (`draw_rect`, `draw_line`, `flood_fill`) for large areas.
- Call `finish` when the artwork is complete.

## Canvas Info
- Current canvas size: {width}x{height}
- Available layers: {layer_count}

## Style Guidelines
- Pixel art should have clean, intentional pixel placement.
- Use limited color palettes (typically 4-16 colors).
- Avoid anti-aliasing; every pixel should be deliberate.
- Consider common pixel art techniques: dithering, outlining, highlights.
```

---

## チャット UI 設計

```
┌─ AI Assistant ──────────────────────────┐
│                                          │
│  ┌────────────────────────────────────┐  │
│  │ 🤖 何を描きましょうか？             │  │
│  │                                    │  │
│  │ 👤 剣のアイコンを描いて             │  │
│  │                                    │  │
│  │ 🤖 32x32の剣アイコンを描きます。    │  │
│  │    まずアウトラインから...           │  │
│  │    [ツール実行中: set_pixels ...]   │  │
│  │    [ツール実行中: flood_fill ...]   │  │
│  │    完成しました！                   │  │
│  │                                    │  │
│  └────────────────────────────────────┘  │
│                                          │
│  ┌────────────────────────────┐ [Send]   │
│  │ メッセージを入力...         │ [Stop]   │
│  └────────────────────────────┘          │
│                                          │
│  Model: claude-sonnet-4-6  ▼             │
│  API Key: ●●●●●●●●●●●●  [設定]          │
└──────────────────────────────────────────┘
```

### UI 要素

| 要素 | 説明 |
|---|---|
| メッセージ領域 | チャット履歴（ユーザー / AI メッセージ + ツール実行ログ） |
| 入力フィールド | テキスト入力 + 送信ボタン |
| 停止ボタン | エージェントループを中断 |
| モデル選択 | sonnet / haiku の選択 |
| API キー設定 | キー入力・保存 (localStorage) |
| ツール実行ログ | 実行中のツール名と進捗をリアルタイム表示 |

### 配置

右パネルにタブとして追加（Layers / AI Chat の切替）。
または、左パネルのツールパネル下部にチャット領域を配置。

---

## メッセージ型定義

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: ChatContent,
    pub timestamp: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ChatRole {
    User,
    Assistant,
    System,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ChatContent {
    Text(String),
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
    Image {
        base64: String,
        media_type: String,
    },
}
```

---

## エージェント状態管理

```rust
pub struct AgentState {
    pub messages: Vec<ChatMessage>,     // 会話履歴
    pub is_running: bool,               // エージェント実行中フラグ
    pub current_turn: usize,            // 現在のターン数
    pub config: AgentConfig,            // 設定
    pub api_key: Option<String>,        // Claude API キー
    pub total_input_tokens: usize,      // 累計入力トークン
    pub total_output_tokens: usize,     // 累計出力トークン
}

impl AgentState {
    /// エージェントループ (async)
    pub async fn run(&mut self, canvas: &mut Canvas, history: &mut History) {
        self.is_running = true;

        for turn in 0..self.config.max_turns {
            self.current_turn = turn;

            // 1. 定期的にキャンバス画像を添付
            if turn % self.config.send_image_every == 0 {
                self.attach_canvas_image(canvas);
            }

            // 2. Claude API 呼び出し
            let response = call_claude_api(
                self.api_key.as_ref().unwrap(),
                &self.messages,
                &TOOL_DEFINITIONS,
                &self.config.model,
            ).await;

            match response {
                Ok(resp) => {
                    self.total_input_tokens += resp.usage.input_tokens;
                    self.total_output_tokens += resp.usage.output_tokens;

                    // 3. レスポンス処理
                    for block in &resp.content {
                        match block {
                            ContentBlock::Text(text) => {
                                self.messages.push(assistant_text(text));
                            }
                            ContentBlock::ToolUse { id, name, input } => {
                                // ツール実行
                                let result = execute_tool(name, input, canvas, history);
                                self.messages.push(tool_result(id, &result));

                                // "finish" ツールで終了
                                if name == "finish" {
                                    self.is_running = false;
                                    return;
                                }
                            }
                        }
                    }

                    // stop_reason == "end_turn" なら終了
                    if resp.stop_reason == "end_turn" {
                        self.is_running = false;
                        return;
                    }
                }
                Err(e) => {
                    self.messages.push(system_error(&e));
                    self.is_running = false;
                    return;
                }
            }
        }

        self.is_running = false; // max_turns 到達
    }
}
```

---

## API コスト見積もり

### 1 回のピクセルアート生成あたり

| 項目 | 見積もり |
|---|---|
| ターン数 | 5〜15 ターン |
| 入力トークン / ターン | ~2,000 (テキスト) + ~1,500 (画像, 32x32 PNG) |
| 出力トークン / ターン | ~500 (ツール呼び出し) |
| 合計入力トークン | ~30,000〜50,000 |
| 合計出力トークン | ~5,000〜10,000 |
| コスト (Sonnet) | ~$0.01〜0.03 / 生成 |
| コスト (Haiku) | ~$0.002〜0.005 / 生成 |

### コスト最適化

- **Haiku をデフォルトに**: 簡単な描画には十分な品質
- **画像送信を制限**: `send_image_every: 3` で 3 ターンに 1 回
- **バッチ描画**: `set_pixels` で複数ピクセルを一度に送信
- **キャンバスサイズ制限**: AI モードでは最大 64x64 を推奨

---

## 実装手順

### Phase 1: API クライアント & ツールブリッジ (基盤)

1. `gloo-net` クレートを依存に追加
2. `app/src/ai/api_client.rs` — Claude API HTTP クライアント
3. `app/src/ai/tools.rs` — ツール定義 JSON + `execute_tool` 関数
4. `app/src/ai/message.rs` — メッセージ型定義
5. 単体テスト（ツール実行のみ、API モックで）

### Phase 2: エージェントループ

1. `app/src/ai/agent.rs` — `AgentState` + `run()` ループ
2. API キーの localStorage 保存/読込
3. エラーハンドリング（ネットワーク、レート制限、無効キー）
4. 停止ボタンによるループ中断

### Phase 3: チャット UI

1. `app/src/components/ai_chat.rs` — チャットパネル
2. メッセージ表示（テキスト + ツール実行ログ）
3. 入力フィールド + 送信/停止ボタン
4. API キー設定ダイアログ
5. 右パネルへのタブ統合

### Phase 4: 描画パフォーマンス & UX

1. ツール実行ごとのリアルタイムキャンバス更新
2. ツール実行中のプログレス表示
3. 履歴統合（AI 操作を Undo 可能に）
4. トークン使用量の表示

### Phase 5: 高度な機能 (将来)

1. 「この画像をピクセルアート化して」（画像入力→AI描画）
2. 「もう少し明るくして」（既存アートの編集指示）
3. プロンプトテンプレート（「RPG アイテム」「キャラクターアイコン」等）
4. 生成結果のギャラリー保存

---

## 依存クレート追加

```toml
# app/Cargo.toml に追加
[dependencies]
gloo-net = "0.6"          # HTTP クライアント (WASM 対応)
serde_json = "1"          # JSON 処理 (既存なら不要)
base64 = "0.22"           # 画像の base64 エンコード
```

---

## 既存コードへの影響

| ファイル | 変更内容 |
|---|---|
| `app/Cargo.toml` | 依存クレート追加 |
| `app/src/main.rs` | AI チャットコンポーネント追加、エージェント状態管理 |
| `app/src/components/mod.rs` | `ai_chat` モジュール追加 |
| `crates/formats/src/png_format.rs` | `export_png` を AI ブリッジから呼び出し（既存 API で十分） |
| `crates/tools/src/lib.rs` | 変更なし（既存関数をそのまま利用） |
| `crates/core/src/lib.rs` | 変更なし |

既存の描画ツール・キャンバスコードへの変更は不要。
新規モジュール (`app/src/ai/`) の追加のみで実装可能。

---

## セキュリティ考慮事項

- API キーはブラウザの localStorage に保存 → ユーザーの自己責任
- API キーは入力フィールドで `type="password"` とし、画面上に露出しない
- Claude API の `anthropic-dangerous-direct-browser-access` ヘッダーが必要
  - このヘッダー名が示す通り、Anthropic 側はブラウザ直接アクセスを推奨していない
  - 本番環境ではプロキシサーバー経由が望ましいが、PWA として完結させるため直接呼び出しを採用
- ツール実行はサンドボックス化されている（Canvas 操作のみ、ファイルシステムアクセスなし）
