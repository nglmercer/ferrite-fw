import { useState } from "preact/hooks";
import { Button } from "../actions/Button.tsx";
import { cn } from "../../lib/cn.ts";

function emailError(value: string): string | null {
  if (value.trim() === "") return "Email is required.";
  if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value)) return "Enter a valid email address.";
  return null;
}

function passwordError(value: string): string | null {
  if (value === "") return "Password is required.";
  if (value.length < 8) return "Use at least 8 characters.";
  return null;
}

/** Live field validation with inline error messaging. */
export function ValidatorDemo() {
  const [email, setEmail] = useState("invalid@example");
  const [password, setPassword] = useState("short");
  const [submitted, setSubmitted] = useState(false);
  const emailIssue = emailError(email);
  const passwordIssue = passwordError(password);
  const valid = emailIssue === null && passwordIssue === null;
  return (
    <form
      className="validator-card"
      noValidate
      onSubmit={event => { event.preventDefault(); setSubmitted(true); }}
    >
      <label className="form-label">Email address
        <input
          className={cn("form-input", emailIssue !== null && "form-input-error")}
          type="email"
          autoComplete="email"
          value={email}
          aria-invalid={emailIssue !== null}
          aria-describedby="validator-email-help"
          onInput={event => setEmail(event.currentTarget.value)}
        />
        <span id="validator-email-help" className={cn("form-help", emailIssue !== null && "validator-error")}>{emailIssue ?? "Looks good."}</span>
      </label>
      <label className="form-label">Password
        <input
          className={cn("form-input", passwordIssue !== null && "form-input-error")}
          type="password"
          value={password}
          autoComplete="new-password"
          aria-invalid={passwordIssue !== null}
          aria-describedby="validator-password-help"
          onInput={event => setPassword(event.currentTarget.value)}
        />
        <span id="validator-password-help" className={cn("form-help", passwordIssue !== null && "validator-error")}>{passwordIssue ?? "Looks good."}</span>
      </label>
      <div className="mt-4 flex items-center gap-3">
        <Button type="submit" variant={valid ? "primary" : "danger"}>Submit with validation</Button>
        {submitted ? <span className="text-sm text-gray-600" aria-live="polite">{valid ? "Submitted successfully." : "Fix the highlighted fields."}</span> : null}
      </div>
    </form>
  );
}
