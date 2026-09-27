const lines = [
  { number: 1, tokens: [{ text: ".button-primary", tone: "token-selector" }, { text: " {", tone: "token-plain" }] },
  { number: 2, tokens: [{ text: "  @apply", tone: "token-directive" }, { text: " bg-brand-600 text-white;", tone: "token-plain" }] },
  { number: 3, tokens: [{ text: "}", tone: "token-plain" }] },
  { number: 4, tokens: [{ text: "", tone: "token-plain" }] },
  { number: 5, tokens: [{ text: "/* deterministic output */", tone: "token-comment" }] }
] as const;

/** Code window with line numbers and tinted tokens. */
export function CodeMockupDemo() {
  return (
    <div className="mockup-code" role="img" aria-label="Code mockup showing an @apply recipe">
      <div className="mockup-code-bar">
        <span className="mockup-dots" aria-hidden="true"><i /><i /><i /></span>
        <span className="mockup-code-file">app.css</span>
      </div>
      <pre className="mockup-code-body" aria-hidden="true"><code>{lines.map(line => (
        <span key={line.number} className="mockup-code-line">
          <span className="mockup-code-number">{line.number}</span>
          {line.tokens.map((token, index) => <span key={index} className={token.tone}>{token.text || " "}</span>)}
        </span>
      ))}</code></pre>
    </div>
  );
}
