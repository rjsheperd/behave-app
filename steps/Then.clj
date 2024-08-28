(ns Then
  (:require
   [clojure.string :as str]
   [cucumber.element :as e]
   [cucumber.by :as by]
   [cucumber.webdriver :as w]
   [cucumber.steps :refer [Then]]))

(defn- extract-submodule-groups
  [submodule-groups]
  (-> submodule-groups
      (str/replace "\"\"\"" "")
      (str/split #"- ")
      (->> (map str/trim)
           (remove empty?)
           (map #(str/split % #" > ")))))

(defn- select-submodule [driver submodule]
  (-> (e/find-el driver (by/css ".wizard"))
      (e/find-el (by/attr= :text submodule))
      (e/click!)))

(defn- navigate-to-inputs [driver]
  (-> (e/find-el driver (by/css ".wizard-header__io-tabs"))
      (e/find-el (by/attr= :text "Inputs"))
      (e/click!)))

(defn- navigate-to-outputs [driver]
  (-> (e/find-el driver (by/css ".wizard-header__io-tabs"))
      (e/find-el (by/attr= :text "Outputs"))
      (e/click!)))

(defn- find-group [driver group]
  (->> (e/find-els driver (by/css ".wizard-group__header"))
       (filter #(= group (.getText %)))
       (first)))

(defn- group-exists? [driver [submodule & groups]]
  (select-submodule driver submodule)
  (let [wait (w/wait driver 5000)]
    (.until wait (w/presence-of (by/css ".wizard-group__header"))))
  (assert (every? some? (map (partial find-group driver) groups))
          (format "ERROR: Groups could not be found: %s > %s" submodule (str/join groups " > "))))

(Then "(?m)the following input Submodule > Groups are displayed: {submodule-groups}"
      (fn [{:keys [driver] :as ctx} submodule-groups]
        (navigate-to-inputs driver)
        (let [wait (w/wait driver 5000)]
          (.until wait (w/presence-of (by/css ".wizard-page__body"))))
        (let [submodule-groups (extract-submodule-groups submodule-groups)]
          (doall (map (partial group-exists? driver) submodule-groups))
          ctx)))

(Then "(?m)the following outupt Submodule > Groups are displayed: {submodule-groups}"
      (fn [{:keys [driver] :as ctx} submodule-groups]
        (navigate-to-outputs driver)
        (let [wait (w/wait driver 5000)]
          (.until wait (w/presence-of (by/css ".wizard-page__body"))))
        (let [submodule-groups (extract-submodule-groups submodule-groups)]
          (assert (every? true? (map (partial group-exists? driver) submodule-groups)))
          ctx)))

(comment
  (count incorrect-groups)
  (pr-str (map pr-str incorrect-groups)))

(comment
  (do
    (require '[cucumber.runner :as r]
             '[cucumber.webdriver :as w])

    (let [d r/driver-atom]
      (e/find-el @d (by/attr= :text "Wind measured at: ")))))
