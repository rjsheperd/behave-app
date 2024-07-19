(ns cucumber.by
  (:import [org.openqa.selenium By]))

(defn class-name
  "Select element by class name."
  [s]
  (By/className s))

(defn css
  "Select element by CSS selector."
  [s]
  (By/cssSelector s))

(defn id
  "Select element by ID."
  [s]
  (By/id s))

(defn input-name
  "Select using an input's `name` attribute."
  [s]
  (By/name s))

(defn link-text
  "Select using an link's text."
  [s]
  (By/linkText s))

(defn partial-link-text
  "Select using an link's partial text."
  [s]
  (By/partialLinkText s))

(defn tag-name
  "Select using a tag."
  [s]
  (By/tagName s))

(defn xpath
  "Select using an element's xpath."
  [s]
  (By/xpath s))
