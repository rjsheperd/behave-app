(ns html2idf.core
  (:require [clojure.string :as str]
            [clojure.walk   :as walk]
            [clojure.java.io :as io]
            [hickory.core   :as hickory]))

(defn html->hiccup [html]
  (first (hickory/as-hiccup (hickory/parse html))))

(def the-html "<html><body style=\"color: red;\"><div>Hello World</div></body></html>")

(def stop-words (-> "a,able,about,across,after,all,almost,also,am,among,an,and,any,are,as,at,be,because,been,but,by,can,cannot,could,dear,did,do,does,either,else,ever,every,for,from,get,got,had,has,have,he,her,hers,him,his,how,however,i,if,in,into,is,it,its,just,least,let,like,likely,may,me,might,most,must,my,neither,no,nor,not,of,off,often,on,only,or,other,our,own,rather,said,say,says,she,should,since,so,some,than,that,the,their,them,then,there,these,they,this,tis,to,too,twas,us,wants,was,we,were,what,when,where,which,while,who,whom,why,will,with,would,yet,you,your"
                    (str/split #",")
                    (set)))

(def non-word-regex
  "regular expression for finding all non-word characters and leaving spaces for delimiting"
  #"(?![a-zA-Z0-9À-ÿ\s])(\W)")

(first stop-words)

(hickory/as-hiccup (hickory/parse the-html))

(defn walk-hiccup-content [fn [tag props & children]]
  (doseq [child children]
    (cond
      (string? child)
      (fn child)

      (vector? child)
      (walk-hiccup-content fn child))))

(defn html->terms [html]
  (let [sentences (atom [])]
    (walk-hiccup-content #(->> %
                               (str/trim)
                               (str/lower-case)
                               (swap! sentences conj))
                         (html->hiccup html))
    (remove #(or (empty? %) (contains? stop-words %)) (-> (str/join " " @sentences)
                                                                (str/replace non-word-regex " ")
                                                                (str/replace #"[0-9]" "")
                                                                (str/split #"\s")))))

(defn calculate-tf [terms]
  (let [c (count terms)
        m (frequencies terms)]
    (reduce-kv (fn [n k v] (assoc n k (float (/ v c)))) {} m)))

(empty {:hello "world"})

(assoc {} "hello" 1)

(update {"hello" 0} "hello" inc)
(html->terms
 (slurp (io/file "../../../behave6/src/DocFolder/en_US/Html/CBD_Dougfir_Lodgepole.html")))

(def the-terms *1)

(sort-by val > (calculate-tf the-terms))
