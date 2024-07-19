(ns cucumber.webdriver
  (:require [cucumber.remote :as remote])
  (:import [org.openqa.selenium By WebDriver]
           [org.openqa.selenium.safari SafariDriver]
           [org.openqa.selenium.chrome ChromeDriver]
           [org.openqa.selenium.firefox FirefoxDriver]
           ;;[org.openqa.selenium JavascriptExecutor]
           [org.openqa.selenium.support.ui WebDriverWait ExpectedConditions]))

(defn goto
  "Navigate to url."
  [^WebDriver d url]
  (.get d url))

(defn presence-of
  "Expect the presence of an element."
  [^By by]
  (ExpectedConditions/presenceOfElementLocated by));

(defn quit
  "Quit the webdriver."
  [^WebDriver driver]
  (.quit driver))

(defn title
  "Get the title of the current website."
  [^WebDriver driver]
  (.getTitle driver))

(defn wait
  "Wait for a given duration."
  [^WebDriver driver duration]
  (WebDriverWait. driver duration))

(defn delete-cookies
  "Deletes all cookies."
  [^WebDriver driver]
  (.. driver (manage) (deleteAllCookies)))

(defn maximize
  "Maxmizes the browser window"
  [^WebDriver d]
  (.. d (manage) (window) (maximize)))

(defn chrome-driver
  "Instatiate a Chrome WebDriver."
  [_]
  (System/setProperty "webdriver.chrome.driver" "/usr/local/bin/chromedriver")
  (ChromeDriver.))

(defn firefox-driver
  "Instatiate a Firefox WebDriver."
  [_]
  (FirefoxDriver.))

(defn safari-driver
  "Instatiate a Safari WebDriver."
  [_]
  (SafariDriver.))

(defn remote-driver
  "Instatiate a remote WebDriver."
  [opts]
  (remote/remote-driver opts))

(defn driver
  "Instantiates a new WebDriver"
  [{:keys [browser remote] :as opts}]
  (println (format "Creating WD -- Remote?: %s Browser: Options: %s" remote opts))
  (if remote
    (remote-driver opts)
    (condp = (keyword browser)
      :chrome (chrome-driver opts)
      :firefox (firefox-driver opts)
      :safari (safari-driver opts))))
