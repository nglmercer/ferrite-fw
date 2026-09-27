import { cn } from "../../lib/cn.ts";

interface ChatMessage {
  readonly author: string;
  readonly time: string;
  readonly text: string;
  readonly own?: boolean;
}

const messages: readonly ChatMessage[] = [
  { author: "Maya Chen", time: "09:41", text: "Morning! Did the nightly build pick up the new button recipes?" },
  { author: "You", time: "09:43", text: "Yes — deterministic output, same bytes on every machine.", own: true },
  { author: "Maya Chen", time: "09:44", text: "Perfect. I will add the banner page to the release notes." }
];

/** Conversation bubbles with start/end alignment for each participant. */
export function ChatBubbleDemo() {
  return (
    <div className="chat-log" role="log" aria-label="Conversation">
      {messages.map(message => (
        <div key={`${message.author}-${message.time}`} className={cn("chat-row", message.own && "chat-row-own")}>
          <span className="chat-avatar" aria-hidden="true">{message.author.split(" ").map(part => part[0]).join("")}</span>
          <div className="chat-message">
            <div className="chat-meta"><strong>{message.author}</strong><time>{message.time}</time></div>
            <p className={cn("chat-bubble", message.own && "chat-bubble-own")}>{message.text}</p>
          </div>
        </div>
      ))}
    </div>
  );
}
