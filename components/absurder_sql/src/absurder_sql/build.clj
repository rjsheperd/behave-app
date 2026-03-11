(ns absurder-sql.build
  (:require [clojure.java.io :as io]
            [clojure.java.shell :as sh]))

(defn- copy-wasm-pkg!
  "Copy persistent-sorted-set WASM binary to the given output directory."
  [output-dir]
  (let [wasm-src (io/file "rust/persistent-sorted-set/pkg/persistent_sorted_set_bg.wasm")
        wasm-dst (io/file output-dir "persistent_sorted_set_bg.wasm")]
    (when (.exists wasm-src)
      (println "Copying PSS WASM binary to" (str output-dir))
      (.mkdirs (io/file output-dir))
      (io/copy wasm-src wasm-dst))))

(defn test-hook
  {:shadow.build/stage :flush}
  [build-state & _args]
  (let [test-dir (get-in build-state [:shadow.build/config :test-dir] "target/test")
        js-dir (io/file test-dir "js")
        src-dir (io/file "resources/public/js")]
    (println "Copying SQLite Files to" test-dir)
    (.mkdirs js-dir)
    (doseq [f ["sqlite.js" "sqlite.wasm" "users.db"]]
      (let [src (io/file src-dir f)]
        (when (.exists src)
          (io/copy src (io/file js-dir f)))))
    (copy-wasm-pkg! js-dir))
  build-state)

(defn datascript-hook
  {:shadow.build/stage :flush}
  [build-state & _args]
  (let [output-dir (get-in build-state [:shadow.build/config :output-dir] "out/ds")]
    (copy-wasm-pkg! (io/file output-dir)))
  build-state)

(defn kaocha-hook
  {:shadow.build/stage :flush}
  [build-state & _args]
  (sh/sh "bin/chrome-refresh")
  (sh/sh "bin/kaocha")
  build-state)
