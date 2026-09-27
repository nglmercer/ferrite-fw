import { useRef, useState } from "preact/hooks";

const OTP_LENGTH = 6;

/** One-time passcode boxes with auto-advance and backspace support. */
export function OtpDemo() {
  const [digits, setDigits] = useState<readonly string[]>(Array(OTP_LENGTH).fill(""));
  const boxes = useRef<Array<HTMLInputElement | null>>([]);
  const complete = digits.every(digit => digit !== "");
  function setDigit(index: number, value: string) {
    const clean = value.replace(/\D/g, "").slice(-1);
    setDigits(current => current.map((digit, position) => position === index ? clean : digit));
    if (clean !== "" && index < OTP_LENGTH - 1) {
      boxes.current[index + 1]?.focus();
    }
  }
  return (
    <div className="space-y-4">
      <div className="otp-row" role="group" aria-label="One-time passcode">
        {digits.map((digit, index) => (
          <input
            key={index}
            ref={element => { boxes.current[index] = element; }}
            className="otp-box"
            inputMode="numeric"
            autoComplete={index === 0 ? "one-time-code" : "off"}
            maxLength={1}
            aria-label={`Digit ${index + 1}`}
            value={digit}
            onInput={event => setDigit(index, event.currentTarget.value)}
            onKeyDown={event => {
              if (event.key === "Backspace" && digits[index] === "" && index > 0) {
                boxes.current[index - 1]?.focus();
              }
            }}
          />
        ))}
      </div>
      <p className="text-sm text-gray-600" aria-live="polite">{complete ? `Code ${digits.join("")} ready to verify.` : "Enter the 6-digit code from your authenticator."}</p>
    </div>
  );
}
