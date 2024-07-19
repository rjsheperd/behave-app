(ns cucumber.runner
  (:require [clojure.java.io :as io]
            [clojure.string :as s]
            [cucumber.webdriver :as w]
            [cucumber.steps :refer [find-step]])
  (:import [java.io File]))

;; Runners

(defn- feature? [line]
  (s/starts-with? line "Feature"))

(defn- scenario? [line]
  (s/starts-with? line "Scenario"))

(defn- comment? [line]
  (s/starts-with? line "#"))

(defn- skip [])

(defn- run-scenario [step-name {:keys [driver]}]
  (println (str "\t\t# " step-name)
  (w/delete-cookies driver)))

(defn- run-step [step fun context]
  (println (str "\t\t\t# " step))
  (try (fun context)
        (catch Exception e (str "Error in " step", caught exception: " (.getMessage e)))))

;; Step processing

(defn- read-steps [^File feature]
  (let [feature-name (.getName feature)
        lines        (->> feature slurp s/split-lines (map s/trim))
        scenarios    (filter scenario? lines)
        steps        (map find-step lines)]
    {:feature feature-name :secenarios scenarios :steps steps}))

(defn- run-feature-steps [context features]
  (doseq [feature features
          {:keys [step-name fun]} (:steps feature)]
    (cond
      (or (empty? step-name) (comment? step-name))
      (skip)

      (feature? step-name)
      (println (str "\t# " step-name))

      (scenario? step-name)
      (run-scenario step-name context)

      :else
      (run-step step-name fun context))))

(defn run-cucumber-tests
  "Runs cucumber tests "
  [{:keys [output features-dir] :as opts}]
  (let [driver        (w/driver opts)
        features-dir  (io/file features-dir)
        feature-files (seq (.listFiles features-dir))
        features      (map #(read-steps %) feature-files)
        context       {:driver driver}]
    ;; Do something with output
    (run-feature-steps context features)
    (w/quit driver)))
