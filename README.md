# Marupo（マルポ） — MarkdownをRustでPDFにするアプリ

**Marupo（マルポ）**（Windows: `Marupo.exe` / Linux: `Marupo`・Flatpak）は、「**Ma**rkdownを**Ru**stで**Po**rtable Document Formatにするアプリ」の頭2文字ずつをとった、純Rust製スタンドアロン・デスクトップアプリケーションです。
WebViewや外部変換ツール（wkhtmltopdf / LaTeX / Chromium等）に一切依存せず、**100%純粋なRustコードのみ**でMarkdownの解析・組版・リアルタイムプレビュー・PDF/HTML出力までを一貫して行います。

---

## 主な特徴

- **完全単体動作（ポータブル設計）**
  - Windowsでは `Marupo.exe` 1ファイルのみで動作します（インストーラー不要・外部ランタイム不要・オフライン完結・アイコン内蔵）。
  - Linuxではネイティブビルド（X11 / Wayland両対応）および **Flatpak** パッケージに対応しています。
- **プレビューと100%一致する出力**
  - 独自の純Rust組版エンジンにより、画面上の縦スクロール連続シートプレビューと出力されるベクターPDFが1ピクセルのズレもなく一致します。
  - 単一ファイルで完結する **スタンドアロンHTML出力（`🌐 HTML出力`）** にも対応しています。
- **複数タブ ＆ ドラッグ＆ドロップ対応**
  - 複数のMarkdownファイル（`.md`, `.markdown`, `.txt`）をウィンドウへドラッグ＆ドロップしてタブで同時に開けます（起動時のコマンドライン引数でのファイル指定にも対応）。
  - 開いている全タブを指定フォルダへ一括でPDF出力する **「📚 一括PDF」** 機能も備えています。
- **システム内フォントの自由な選択**
  - OS内にインストールされているフォント（Windows: 游ゴシック、游明朝、メイリオ等 / Linux: Noto Sans CJK JP、Noto Serif CJK JP、IPAexゴシック等）を検索・選択できます。
- **強調・マーカー・見出しの自在なカスタマイズ**
  - 太字（`**Bold**`）、蛍光マーカー（`==Highlight==`）、斜体（`*Italic*`）、インラインコード（`` `code` ``）の **文字色・背景色・文字サイズ倍率（0.75x〜1.80x）・太さ・下線・傾き** をGUIスライダーでリアルタイムに調整できます。
- **プロ向け JSON / CSS ライブ編集**
  - テーマ設定を JSON ファイル（`.json`）または CSS スタイルシート（`.css`）として直接編集・インポート・エクスポートできます（`.json` / `.css` ファイルのドラッグ＆ドロップ適用にも対応）。
- **初期設定のカスタマイズ保存**
  - お気に入りのフォントや配色・UIモード（ダーク／ライト）・ズーム率を「★ 初期値に保存」ボタンで次回起動時のデフォルトとして登録できます。

---

## 対応しているMarkdown記法

標準の **CommonMark** に加え、以下の拡張記法にネイティブ対応しています。

1. **YAML Frontmatter（メタデータ／プロパティ）**
   - 文頭の `---` で囲まれたメタデータを解析し、**「非表示」「ヘッダー形式」「テーブル形式」** の3通りで表示を切り替えられます。
2. **自動目次生成（TOC）**
   - 設定の「目次 (TOC) を自動挿入」をONにするか、本文中に `[TOC]` と記述すると、見出し（H1〜H4）から自動でリーダー線付き目次を生成します。
3. **コールアウト（GitHub風アラートブロック）**
   - `> [!NOTE]`, `> [!TIP]`, `> [!IMPORTANT]`, `> [!WARNING]`, `> [!CAUTION]` に対応し、アイコン・アクセントバー・背景色付きのボックスとして組版します。
4. **蛍光ペン・ハイライト記法**
   - `==ハイライトしたい文字列==` で背景マーカーを引けます（色・サイズ倍率変更可）。
5. **数式（LaTeX Math記法）**
   - インライン数式 `$E = mc^2$` およびディスプレイ数式 `$$...$$` に対応しています。
   - 分数 `\frac{a}{b}`、平方根 `\sqrt{x}`、上付き・下付き添字 `x_{i}^{2}`、ギリシャ文字（`\alpha`, `\beta`, `\pi`, `\sum`, `\int` 等）を2次元組版します。
6. **GFM拡張（テーブル・タスクリスト・取り消し線・脚注）と直感的な改行処理**
   - 表組み（左寄せ・中央・右寄せ）、チェックボックス `- [x] / - [ ]`、取り消し線 `~~text~~`、脚注 `[^1]` に対応しています。
   - **改行の扱い**: 初期設定では、通常の単一改行（Enter 1回）もそのまま改行として反映されます。設定パネルの「📌 メタデータ・目次・改行」から **「厳密な改行にする（行末スペース2つで改行）」** にいつでも切り替え可能です。

---

## 使い方

### 1. GUIモード（通常起動）
1. `Marupo.exe` をダブルクリックして起動します。
2. 上部の **「📂 開く」** ボタン、または `.md` ファイルをウィンドウへ **ドラッグ＆ドロップ** して読み込みます。
3. 左側パネルの **「🎨 デザイン」** タブで各設定カードをクリックして開き、フォント・強調スタイル・見出し・用紙サイズ（A4 / JIS B5 / Letter、縦・横、余白）を調整します。
   - 画面上部の **「テーマ」** プルダウンから、用途に応じた4種類のプリセット（標準 / 論文・明朝 / 強調重視 / 書籍風）をワンクリックで切り替えられます。
4. 右上の **「📄 PDF出力」** または **「🌐 HTML出力」** をクリックして保存します。

### 2. コマンドライン（CLI）モード
スクリプトやバッチ処理から直接PDFやHTMLを生成することも可能です。

```powershell
# 標準テーマでPDFへ変換
.\Marupo.exe --export input.md output.pdf

# 標準テーマでHTMLへ変換
.\Marupo.exe --export input.md output.html

# カスタムJSONテーマまたはCSSを指定して変換
.\Marupo.exe --export input.md output.pdf --theme my_theme.json
.\Marupo.exe --export input.md output.pdf --theme my_style.css
```

---

## 設定ファイルの保存場所とアンインストール

- 「★ 初期値に保存」で登録したユーザー初期設定・ウィンドウ状態の保存先:
  - **Windows**: 実行フォルダ内の `user_defaults.json`、ウィンドウ状態は `%APPDATA%\Marupo\`
  - **Linux（ネイティブ）**: `~/.config/marupo/user_defaults.json`、ウィンドウ状態は `~/.local/share/io.github.negxkuniklo.marupo/`
  - **Linux（Flatpak）**: `~/.var/app/io.github.negxkuniklo.Marupo/config/marupo/user_defaults.json`、ウィンドウ状態は `~/.var/app/io.github.negxkuniklo.Marupo/data/io.github.negxkuniklo.marupo/`
- アプリケーションをアンインストールする場合:
  - Windowsでは `Marupo.exe` 本体と設定ファイルを削除するだけで完了します（レジストリ等は一切変更しません）。
  - Flatpakでは `flatpak uninstall io.github.negxkuniklo.Marupo` で削除できます。

---

## Linux / Flatpak でのビルド・インストール

### 1. Flatpak（推奨・サンドボックス実行）

```bash
# ビルドして Marupo.flatpak を生成
./packaging/flatpak/build-flatpak.sh

# ローカルへインストールして起動
flatpak install --user -y target/Marupo.flatpak
flatpak run io.github.negxkuniklo.Marupo

# Flatpak経由でCLI変換を実行する場合
flatpak run io.github.negxkuniklo.Marupo --export input.md output.pdf
```

> `Cargo.lock` を更新した場合は、`python3 packaging/flatpak/gen-cargo-sources.py` を実行して `packaging/flatpak/cargo-sources.json` を再生成してください。

### 2. Linux ネイティブインストール

```bash
# ビルドして ~/.local 以下にバイナリ・アイコン・.desktop を配置
./packaging/linux/install.sh
```

> 日本語表示のために `fonts-noto-cjk` などの日本語フォントがシステムにインストールされていることを推奨します。

---

## 動作環境

- **OS**:
  - Windows 10 / Windows 11 (64-bit)
  - Linux (X11 / Wayland, Flatpak `org.freedesktop.Platform//24.08` 対応)
- **ライセンス**: MIT License（詳細は同梱の `LICENSE.txt` をご参照ください）
