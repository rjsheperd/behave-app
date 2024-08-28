(ns When
  (:require
   [cucumber.by :as by]
   [cucumber.element :as e]
   [cucumber.steps :refer [When]]
   [clojure.string :as str]
   [cucumber.runner :as r]
   [cucumber.webdriver :as w]))

(defn- extract-submodule-groups
  [submodule-groups]
  (-> submodule-groups
      (str/replace "\"\"\"" "")
      (str/split #"- ")
      (->> (map str/trim)
           (remove empty?)
           (map #(str/split % #" > ")))))

(defn- select-submodule [driver submodule]
  (-> (e/find-el driver (by/css ".wizard-header__submodules"))
      (e/find-el (by/attr= :text submodule))
      (e/click!)))

(defn- find-group [driver group]
  (->> (e/find-els driver (by/css ".wizard-group__header"))
       (filter #(= group (.getText %)))
       (first)))

(defn- select-output! [driver output]
  (-> (e/find-el driver (by/xpath ".."))
      (e/find-els (by/css ".wizard-group__outputs .input-checkbox__label"))
      (->> (filter #(= output (.getText %))))
      (first)
      (e/click!)))

(defn- select-submodule-and-output [driver [submodule & groups]]
  (select-submodule driver submodule)
  (let [wait   (w/wait driver 5000)
        output (last groups)
        groups (butlast groups)]
    (.until wait (w/presence-of (by/css ".wizard")))
    (-> (find-group driver (first groups))
          (select-output! output))))

(defn- select-submodule-and-outputs
  [{:keys [driver]} submodule-groups]
  (let [wait (w/wait driver 5000)]
    (.until wait (w/presence-of (by/css ".wizard"))))
  (let [submodule-groups (extract-submodule-groups submodule-groups)]
    (doseq [output submodule-groups]
      (select-submodule-and-output driver output))
    {:driver driver}))

(When "I select these outputs Submodule > Group > Output: {outputs}" select-submodule-and-outputs)

(comment
  (do
    (require '[cucumber.runner :as r]
             '[cucumber.webdriver :as w])

    (def d @r/driver-atom)
    (def driver @r/driver-atom)

    (select-submodule-and-output d ["Fire Behavior" "Fire Perimeter" "Fire Perimeter"])
    (select-submodule-and-output d ["Size" "Fire Perimeter" "Fire Perimeter"])
    (select-submodule-and-output d ["Size" "Fire Area" "Fire Area"])
    (select-submodule-and-output d ["Size" "Spread Distance" "Spread Distance"])

      ))
