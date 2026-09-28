import { useState } from "react";
import { Layers3, Save, ArrowRight } from "lucide-react";
import { profileName, type Snapshot } from "./shared/contracts";
import type { Mutate } from "./Mixer";
export default function Profiles({
  snapshot,
  busy,
  mutate,
}: {
  snapshot: Snapshot | null;
  busy: boolean;
  mutate: Mutate;
}) {
  const [name, setName] = useState("");
  const valid = profileName.safeParse(name.trim()).success;
  return (
    <section className="profiles-page">
      <div className="section-heading">
        <div>
          <h2>A mix for every moment.</h2>
          <p>Save your current groups, application assignments and balance.</p>
        </div>
        <Layers3 size={27} />
      </div>
      <form
        className="profile-form"
        onSubmit={(event) => {
          event.preventDefault();
          if (valid) void mutate((b) => b.saveProfile(name.trim()), "Profile saved");
        }}
      >
        <label>
          Profile name
          <input
            value={name}
            maxLength={80}
            placeholder="Music and calls"
            onChange={(e) => setName(e.target.value)}
            aria-describedby="profile-name-help"
          />
        </label>
        <button className="primary" disabled={!snapshot || snapshot.readOnly || busy || !valid}>
          <Save size={17} />
          Save current mix
        </button>
      </form>
      <p id="profile-name-help" className="helper">
        Up to 80 UTF-8 bytes (some characters use more than one); control characters are not allowed. Saving an existing name replaces that profile.
      </p>
      <div className="profile-list">
        {snapshot?.profiles.length ? (
          snapshot.profiles.map((profile) => (
            <div className="profile-row" key={profile.name}>
              <Layers3 size={22} />
              <div>
                <h3>{profile.name}</h3>
                <p>Saved locally by the audio service</p>
              </div>
              <button
                disabled={busy || snapshot.readOnly || !snapshot.audio.available}
                aria-label={`Apply ${profile.name}`}
                onClick={() => void mutate((b) => b.applyProfile(profile.name), "Profile applied")}
              >
                Apply
                <ArrowRight size={16} />
              </button>
            </div>
          ))
        ) : (
          <div className="empty-state">
            <Layers3 size={30} />
            <h3>No saved profiles yet</h3>
            <p>Set up your mixer, then save it here. Applying a profile changes your current audio mix.</p>
          </div>
        )}
      </div>
    </section>
  );
}
