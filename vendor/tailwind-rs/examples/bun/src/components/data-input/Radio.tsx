import { useState } from "preact/hooks";

const plans = [
  { value: "starter", label: "Starter", hint: "3 projects · community support" },
  { value: "pro", label: "Pro", hint: "Unlimited projects · priority support" },
  { value: "enterprise", label: "Enterprise", hint: "SAML SSO · dedicated support" }
];

/** Card-style radio group for a single choice. */
export function RadioDemo() {
  const [plan, setPlan] = useState("pro");
  return (
    <div className="radio-cards" role="radiogroup" aria-label="Billing plan">
      {plans.map(entry => (
        <label key={entry.value} className={`radio-card ${plan === entry.value ? "radio-card-checked" : ""}`}>
          <input
            className="sr-only"
            type="radio"
            name="plan"
            value={entry.value}
            checked={plan === entry.value}
            onChange={() => setPlan(entry.value)}
          />
          <strong>{entry.label}</strong>
          <span>{entry.hint}</span>
        </label>
      ))}
    </div>
  );
}
