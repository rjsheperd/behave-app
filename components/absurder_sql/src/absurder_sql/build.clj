(ns absurder-sql.build
  (:require [clojure.java.io :as io]
            [clojure.java.shell :as sh]))

(def ^:private pss-dir "rust/persistent-sorted-set")

(defn- newest-mtime
  "Return the newest last-modified time (ms) among files matching `pred`
   under `dir`, recursively."
  [dir pred]
  (->> (file-seq (io/file dir))
       (filter #(and (.isFile %) (pred (.getName %))))
       (map #(.lastModified %))
       (reduce max 0)))

(defn- pss-wasm-stale?
  "True when any Rust source file is newer than the compiled WASM binary."
  []
  (let [wasm-file (io/file pss-dir "pkg/persistent_sorted_set_bg.wasm")]
    (or (not (.exists wasm-file))
        (let [wasm-mtime (.lastModified wasm-file)
              src-mtime  (newest-mtime (io/file pss-dir "src")
                                       #(.endsWith % ".rs"))]
          (> src-mtime wasm-mtime)))))

(defn- patch-import-meta!
  "Replace `import.meta.url` in wasm-pack output with a dummy string.
   Closure Compiler cannot transpile import.meta, but we never hit that
   code path because CLJS always passes an explicit WASM URL."
  []
  (let [f   (io/file pss-dir "pkg/persistent_sorted_set.js")
        src (slurp f)]
    (when (.contains src "import.meta.url")
      (spit f (.replace src
                        "new URL('persistent_sorted_set_bg.wasm', import.meta.url)"
                        "'persistent_sorted_set_bg.wasm'")))))

(defn- build-pss-wasm!
  "Run wasm-pack build for the persistent-sorted-set crate.
   Returns true on success."
  []
  (println "Building PSS WASM (rust sources changed)...")
  (let [{:keys [exit out err]}
        (sh/sh "wasm-pack" "build" "--target" "web" "--no-typescript"
               :dir pss-dir)]
    (when (seq out) (println out))
    (when (seq err) (println err))
    (when-not (zero? exit)
      (throw (ex-info "wasm-pack build failed" {:exit exit})))
    (patch-import-meta!)
    true))

(defn wasm-hook
  "Shadow-cljs build hook (:compile-prepare stage).
   Rebuilds the PSS WASM binary when Rust sources have changed."
  {:shadow.build/stage :compile-prepare}
  [build-state & _args]
  (when (pss-wasm-stale?)
    (build-pss-wasm!))
  build-state)

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
