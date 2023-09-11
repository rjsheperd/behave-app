(ns behave-cms.socket)

(defonce ^:private socket (atom nil))

(defn connect!
  "Connect to WebSocket on current host."
  [{:keys [on-open on-close on-message]}]
  (let [host   (.-host js/location)]
    (reset! socket (js/WebSocket. (str "ws://" host "/happiness")))
    (when on-open (set! (.-onopen @socket) on-open))
    (when on-close (set! (.-onclose @socket) on-close))
    (when on-message (set! (.-onmessage @socket) on-message))))

(defn send!
  "Send a message to open socket."
  [message]
  (when @socket 
    (.send @socket message)))
