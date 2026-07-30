import { AuthModal } from "@/components/auth/auth-modal";
import type { AuthMode } from "@/lib/auth";

const flowSteps = [
  {
    number: "01",
    title: "Drop it in",
    description:
      "Save the thought before it disappears — a note, a photo, or a document.",
  },
  {
    number: "02",
    title: "Make it clear",
    description:
      "Life Inbox finds the tasks, dates, context, and unanswered questions.",
  },
  {
    number: "03",
    title: "Move forward",
    description:
      "Review the plan, choose your next action, and leave the rest for later.",
  },
];

const signals = [
  "Landlord renewal form — Friday",
  "Book a dentist appointment",
  "Check insurance renewal date",
];

type HomePageProps = {
  searchParams: Promise<{ auth?: string | string[] }>;
};

export default async function HomePage({ searchParams }: HomePageProps) {
  const { auth } = await searchParams;
  const authMode: AuthMode | null =
    auth === "sign-in" ? "sign-in" : auth === "sign-up" ? "sign-up" : null;

  return (
    <>
      <main>
        <nav aria-label="Main navigation" className="site-nav">
          <a aria-label="Life Inbox home" className="brand" href="#top">
            <span aria-hidden="true" className="brand-mark">
              L
            </span>
            <span>Life Inbox</span>
          </a>

          <div className="nav-links">
            <a href="#how-it-works">How it works</a>
            <a href="#principles">Why it feels different</a>
          </div>

          <div className="nav-actions">
            <a className="nav-sign-in" href="/?auth=sign-in">
              Sign in
            </a>
            <a
              className="button button-small button-primary"
              href="/?auth=sign-up"
            >
              Start your Inbox
            </a>
          </div>
        </nav>

        <section className="hero" id="top">
          <div className="hero-copy">
            <p className="eyebrow">
              <span aria-hidden="true" className="eyebrow-dot" />
              Your life, less scattered
            </p>
            <h1>Turn life clutter into one clear next action.</h1>
            <p className="hero-intro">
              Life Inbox turns the thoughts, reminders, and loose ends in your
              head into a calm, practical plan — always reviewed by you first.
            </p>
            <div className="hero-actions">
              <a className="button button-primary" href="/?auth=sign-up">
                Start your Inbox
                <span aria-hidden="true">↘</span>
              </a>
              <a className="text-link" href="#principles">
                Built around your control <span aria-hidden="true">→</span>
              </a>
            </div>
          </div>

          <div className="hero-art" id="preview">
            <div className="sun" />
            <div className="leaf leaf-one" />
            <div className="leaf leaf-two" />
            <div className="leaf leaf-three" />

            <div className="app-window">
              <div className="app-topbar">
                <span className="mini-brand">
                  <span aria-hidden="true" className="mini-brand-mark">
                    L
                  </span>
                  Life Inbox
                </span>
                <span className="avatar">ME</span>
              </div>

              <div className="app-body">
                <div className="app-heading-row">
                  <div>
                    <p className="app-kicker">Tuesday, 30 July</p>
                    <h2>Today</h2>
                  </div>
                  <span className="soft-icon" aria-hidden="true">
                    +
                  </span>
                </div>

                <section className="next-action-card">
                  <div className="card-label-row">
                    <span className="card-label">Your next action</span>
                    <span className="time-pill">10 min</span>
                  </div>
                  <p>Reply to your landlord about the renewal form.</p>
                  <div className="next-action-footer">
                    <span>Due Friday</span>
                    <span aria-hidden="true" className="circle-arrow">
                      ↗
                    </span>
                  </div>
                </section>

                <div className="app-list-header">
                  <span>Your plan</span>
                  <span>2 of 3</span>
                </div>
                <ul className="plan-list">
                  {signals.map((signal, index) => (
                    <li key={signal}>
                      <span
                        aria-hidden="true"
                        className={index === 0 ? "check complete" : "check"}
                      >
                        {index === 0 ? "✓" : ""}
                      </span>
                      <span>{signal}</span>
                      {index === 2 ? (
                        <span className="waiting">Waiting</span>
                      ) : null}
                    </li>
                  ))}
                </ul>
              </div>
            </div>

            <div className="capture-card">
              <span className="capture-spark" aria-hidden="true">
                ✦
              </span>
              <p>“Remember to sort the insurance thing…”</p>
              <span>Captured just now</span>
            </div>
          </div>
        </section>

        <section className="trust-bar" aria-label="Product promises">
          <p>Made for the things that live in your head.</p>
          <div>
            <span>No autopilot</span>
            <span>•</span>
            <span>Your review first</span>
            <span>•</span>
            <span>One step at a time</span>
          </div>
        </section>

        <section className="flow-section" id="how-it-works">
          <div className="section-intro">
            <p className="eyebrow">A gentler way to get organised</p>
            <h2>From “I should remember this” to “I know what to do.”</h2>
          </div>

          <ol className="flow-grid">
            {flowSteps.map((step) => (
              <li key={step.number}>
                <span className="step-number">{step.number}</span>
                <h3>{step.title}</h3>
                <p>{step.description}</p>
              </li>
            ))}
          </ol>
        </section>

        <section className="principles-section" id="principles">
          <div className="principles-copy">
            <p className="eyebrow">A plan you can trust</p>
            <h2>
              Helpful enough to move you forward. Quiet enough to feel like
              yours.
            </h2>
            <p>
              Life Inbox does the sorting, so you can spend less energy holding
              things together. But it never mistakes a suggestion for your
              decision.
            </p>
            <a className="text-link" href="#top">
              Back to the top <span aria-hidden="true">↑</span>
            </a>
          </div>

          <div className="principle-cards">
            <article className="principle-card principle-card-dark">
              <span className="principle-icon" aria-hidden="true">
                ✓
              </span>
              <h3>You stay in control</h3>
              <p>Edit every suggestion before it becomes a plan.</p>
            </article>
            <article className="principle-card principle-card-light">
              <span className="principle-icon" aria-hidden="true">
                ⟡
              </span>
              <h3>Space for real life</h3>
              <p>See what is waiting, what matters now, and what can wait.</p>
            </article>
          </div>
        </section>

        <section className="closing-section">
          <p className="eyebrow">A calmer starting point</p>
          <h2>Your next action is closer than you think.</h2>
          <a className="button button-primary" href="/?auth=sign-up">
            Create your workspace <span aria-hidden="true">↗</span>
          </a>
        </section>

        <footer>
          <a aria-label="Life Inbox home" className="brand" href="#top">
            <span aria-hidden="true" className="brand-mark">
              L
            </span>
            <span>Life Inbox</span>
          </a>
          <p>Make room for what matters.</p>
        </footer>
      </main>
      {authMode ? <AuthModal initialMode={authMode} /> : null}
    </>
  );
}
