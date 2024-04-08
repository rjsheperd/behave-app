#!/usr/bin/env bb

(ns sysctl)

(require '[babashka.process :refer [shell]])

(defn sysctl-cmd [cmd]
  (shell (format "systemctl --user %s" cmd)))

(defn reload!
  "Reload user's systemd services."
  [] (sysctl-cmd "daemon-reload"))

(defn enable!
  "Start user's `service`."
  [service]
  (sysctl-cmd (format "enable %s" service)))

(defn start!
  "Start user's `service`."
  [service]
  (sysctl-cmd (format "start %s" service)))

(defn stop!
  "Stop user's `service`."
  [service]
  (sysctl-cmd (format "stop %s" service)))

(defn status
  "Status of user's `service`."
  [service]
  (sysctl-cmd (format "status %s" service)))
