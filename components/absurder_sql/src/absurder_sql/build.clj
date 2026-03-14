(ns absurder-sql.build
  (:require [clojure.java.io :as io]
            [clojure.java.shell :as sh]))

(def ^:private wasm-crate-dir "rust/datascript-rs")
(def ^:private wasm-pkg-name "datascript_rs")

(defn- newest-mtime
  "Return the newest last-modified time (ms) among files matching `pred`
   under `dir`, recursively."
  [dir pred]
  (->> (file-seq (io/file dir))
       (filter #(and (.isFile %) (pred (.getName %))))
       (map #(.lastModified %))
       (reduce max 0)))

(defn- wasm-stale?
  "True when any Rust source file is newer than the compiled WASM binary."
  []
  (let [wasm-file (io/file wasm-crate-dir (str "pkg/" wasm-pkg-name "_bg.wasm"))]
    (or (not (.exists wasm-file))
        (let [wasm-mtime (.lastModified wasm-file)
              ;; Check both PSS and datascript-rs sources
              pss-mtime  (newest-mtime (io/file "rust/persistent-sorted-set/src")
                                       #(.endsWith % ".rs"))
              ds-mtime   (newest-mtime (io/file wasm-crate-dir "src")
                                       #(.endsWith % ".rs"))]
          (or (> pss-mtime wasm-mtime)
              (> ds-mtime wasm-mtime))))))

(defn- patch-import-meta!
  "Replace `import.meta.url` in wasm-pack output with a dummy string.
   Closure Compiler cannot transpile import.meta, but we never hit that
   code path because CLJS always passes an explicit WASM URL."
  []
  (let [f   (io/file wasm-crate-dir (str "pkg/" wasm-pkg-name ".js"))
        src (slurp f)]
    (when (.contains src "import.meta.url")
      (spit f (.replace src
                        (str "new URL('" wasm-pkg-name "_bg.wasm', import.meta.url)")
                        (str "'" wasm-pkg-name "_bg.wasm'"))))))

(defn- build-wasm!
  "Run wasm-pack build for the datascript-rs unified crate.
   Returns true on success."
  []
  (println "Building unified WASM (rust sources changed)...")
  (let [{:keys [exit out err]}
        (sh/sh "wasm-pack" "build" "--target" "web" "--no-typescript"
               :dir wasm-crate-dir)]
    (when (seq out) (println out))
    (when (seq err) (println err))
    (when-not (zero? exit)
      (throw (ex-info "wasm-pack build failed" {:exit exit})))
    (patch-import-meta!)
    true))

(defn wasm-hook
  "Shadow-cljs build hook (:compile-prepare stage).
   Rebuilds the unified WASM binary when Rust sources have changed."
  {:shadow.build/stage :compile-prepare}
  [build-state & _args]
  (when (wasm-stale?)
    (build-wasm!))
  build-state)

(defn- copy-wasm-pkg!
  "Copy unified WASM binary to the given output directory."
  [output-dir]
  (let [wasm-src (io/file wasm-crate-dir (str "pkg/" wasm-pkg-name "_bg.wasm"))
        wasm-dst (io/file output-dir (str wasm-pkg-name "_bg.wasm"))]
    (when (.exists wasm-src)
      (println "Copying unified WASM binary to" (str output-dir))
      (.mkdirs (io/file output-dir))
      (io/copy wasm-src wasm-dst))))

(defn test-hook
  {:shadow.build/stage :flush}
  [build-state & _args]
  (let [test-dir (get-in build-state [:shadow.build/config :test-dir] "target/test")
        js-dir (io/file test-dir "js")]
    (.mkdirs js-dir)
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
