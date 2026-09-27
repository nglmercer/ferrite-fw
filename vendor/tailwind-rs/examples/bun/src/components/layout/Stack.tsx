/** Cards layered with a fanned offset behind the top surface. */
export function StackDemo() {
  return (
    <div className="stack-demo">
      <div className="stack" aria-label="Stacked notifications">
        <div className="stack-card stack-card-back" aria-hidden="true" />
        <div className="stack-card stack-card-middle" aria-hidden="true" />
        <div className="stack-card stack-card-front">
          <strong>3 notifications</strong>
          <p>Your weekly review is ready to read.</p>
        </div>
      </div>
      <div className="stack stack-images" aria-label="Stacked previews">
        <span className="stack-image stack-image-one" aria-hidden="true">A</span>
        <span className="stack-image stack-image-two" aria-hidden="true">B</span>
        <span className="stack-image stack-image-three" aria-hidden="true">C</span>
      </div>
    </div>
  );
}
