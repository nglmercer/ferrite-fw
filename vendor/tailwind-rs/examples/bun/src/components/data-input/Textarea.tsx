import { useState } from "preact/hooks";

const MAX_LENGTH = 280;

/** Resizable message box with a live character count. */
export function TextareaDemo() {
  const [message, setMessage] = useState("Tell us what you are building…");
  const remaining = MAX_LENGTH - message.length;
  return (
    <div>
      <label className="form-label">Message
        <textarea
          className="form-input form-textarea"
          name="message"
          rows={4}
          maxLength={MAX_LENGTH}
          value={message}
          onInput={event => setMessage(event.currentTarget.value)}
        />
      </label>
      <p className="form-help" aria-live="polite">{remaining} characters remaining.</p>
    </div>
  );
}
