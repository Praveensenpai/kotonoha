# Rule: Japanese Subtitle Mining & Sudachi POS Requirements

1. **Sudachi Content Word POS Hierarchy**:
   - Must include `名詞` (Nouns), `代名詞` (Pronouns: 私, 僕, 俺), `接頭辞` (Prefixes: 小, 大, 超), `動詞` (Verbs), `形容詞` (i-adjectives), `形状詞` (na-adjectives: 好き, 静か), `副詞` (Adverbs), and `連体詞` (Adnominals).
2. **Single-Character Filter**:
   - Exclude single-kana fillers (`そ`, `ア`), but allow single-kanji content words (`仲`, `愛`, `心`).
3. **Subtitle Text Flattening**:
   - Flatten all literal newlines (`\n`, `\r`, `\N`) into single spaces so subtitle text stays on a single terminal row without breaking layouts.
