(ns datomic)

;;; Dependencies

(require '[babashka.deps :as deps])

(deps/add-deps ' {sig-gis/triangulum {:git/url "https://github.com/sig-gis/triangulum"
                                      :git/sha "57d309ea452d756d6485bfaeccdd11665a5ffa2b"}})

(require '[babashka.curl    :as curl])
(require '[babashka.fs      :as fs])
(require '[babashka.process :refer [shell]])
(require '[clojure.java.io  :as io])
(require '[clojure.string   :as str])
(require '[triangulum.utils :refer [format-with-dict path]])

;;; Constants

(def -version     "1.0.7075")
(def -datomic     (path (fs/home) ".datomic-2"))
(def -datomic-bin (path -datomic "current" "bin"))
(def -shell-rc    (let [user-shell   (System/getenv "SHELL")
                        shell-config {"/bin/zsh"  "~/.zshrc"
                                      "/bin/bash" "~/.bashrc"}]
                    (shell-config user-shell)))

(def -bb-path     (or (-> (shell {:out :string} "which bb")
                          (:out)
                          (str/split-lines)
                          (first))
                      "/usr/local/bin/bb"))

(defn install! []
  (let [datomic-zip (format "datomic-pro-%s.zip" -version)
        zip-file    (path -datomic datomic-zip)
        current     (path -datomic "current")
        final-dir   (path -datomic (format "datomic-pro-%s" -version))]

    ;; Create ~/.datomic
    (println "Creating dir:" -datomic)
    (fs/create-dirs -datomic)

    ;; Download
    (println "Downloading... (this will take a while)")
    (curl/get (format "https://datomic-pro-downloads.s3.amazonaws.com/%s/%s" -version datomic-zip)
              {:raw-args ["--progress-bar" "-o" zip-file]})

    ;; Unzip
    (println "Unzipping")
    (shell (format "unzip -q -o %s -d %s"
                   zip-file
                   -datomic))

    ;; Symlink
    (println "Symlinking to ~/.datomic/current")
    (fs/create-sym-link current final-dir)

    ;; Add to PATH
    (println "Install complete! Execute the following to add it to Datomic to your PATH:")
    (println
     (format "echo 'export PATH=$HOME/.datomic/current/bin:$PATH' >> %s" -shell-rc))))

(def service-template "
[Unit]
Description=Datomic Transactor

[Service]
Type=simple
WorkingDirectory={{cwd}}
ExecStart={{bb}} transactor
Restart=always
RestartSec=100

# Logging
StandardOutput=append:{{log-dir}}/transactor-output.log
StandardError=append:{{log-dir}}/transactor-error.log

[Install]
WantedBy=default.target")

;;; Service setup
(defn enable-service!
  []
  (let [service-name "datomic-transactor"
        service-file (format "%s.service" service-name)
        log-dir      (path (fs/cwd) "logs")
        service-file (path (fs/xdg-config-home) "systemd" "user" service-file)
        config       {:cwd     (str (fs/cwd))
                      :bb      -bb-path
                      :log-dir log-dir}
        contents     (format-with-dict service-template config)]
    (fs/create-dirs service-file)
    (fs/create-dirs log-dir)
    (spit service-file contents)
    (sysctl/reload!)
    (sysctl/start! (str/replace tx-service ".service" ""))))

(defn transactor []
  (let [transactor-bin (path -datomic-bin "transactor")
        options        "-Ddatomic.printConnectionInfo=true"
        config         (path (fs/cwd) "config" "datomic-sql.properties")]
    (shell
     (format "%s %s %s" transactor-bin options config))))

(defn console []
  (let [console-bin (io/file -datomic-bin "console")
        port        8000
        url         "datomic:sql://?jdbc:postgresql://localhost:5432/datomic?user=datomic&password=datomic"]
    (shell (format "%s -p %d db %s" console-bin port url))))
