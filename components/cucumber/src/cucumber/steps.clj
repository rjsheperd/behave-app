(ns cucumber.steps)

(def ^:private all-steps (atom []))

;; Private

(defn- step-maker [phrase f]
  (swap! all-steps conj {:match (re-pattern phrase) :fun f}))

(defn- unknown-def [step-name]
  (fn [_] (println "Could not find definition for: '" step-name "'")))

;; Public
(defn find-step
  "Finds a step matching `step-name`, which is a regex."
  [step-name]
  (let [step-defs (filter #(re-find (:match %) step-name) @all-steps)
        fun (-> (or (first step-defs) {}) (get :fun (unknown-def step-name)))]
    {:step-name step-name :fun fun}))

(defn run-step
  "Runs a step matching `step-name` with context `ctx`. `step-name` can be a regex."
  [step-name ctx]
  (let [{:keys [fun]} (find-step step-name)]
    (fun ctx)))

;; Steps
(defn Given
  "Step definition."
  [phrase f]
  (step-maker phrase f))

(defn When
  "Step definition."
  [phrase f]
  (step-maker phrase f))

(defn Then
  "Step definition."
  [phrase f]
  (step-maker phrase f))

(defn And
  "Step definition."
  [phrase f]
  (step-maker phrase f))

