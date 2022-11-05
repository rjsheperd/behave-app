(ns triple-store.core
  (:require [clojure.java.io :as io]
            [arachne.aristotle :as aa]
            [arachne.aristotle.registry :as reg]
            [arachne.aristotle.graph :as g]
            [arachne.aristotle.query :as q])
  (:import '[org.apache.jena.riot RDFDataMgr RDFFormat Lang]))

(defn new-graph []
  (aa/graph :jena-mini))

(defn write-graph [graph file & [format]]
  (if (= :edn format)
    (spit file (prn-str (g/graph->clj @triples)))
    (RDFDataMgr/write (io/output-stream file) graph (or Lang/NTRIPLES format))))

(def prefixes
  {'fsv.* "http://lod.fs.usda.gov/vocab/"
   'bhp.* "http://behave.fs.usda.gov/"
   'foaf "http://xmlns.com/foaf/0.1/"
   'qb "http://purl.org/linked-data/cube#"
   'qudt "http://qudt.org/2.1/schema/qudt"})

(comment
  (def triples (atom (new-graph)))

  (reg/with prefixes
    (aa/read @triples (io/file "ontologies/vocab.ttl"))

    (aa/add @triples {:rdf/about :bhp.module/Surface :rdf/type :fsv/Module})
    (aa/add @triples {:rdf/about :bhp.module/Contain :rdf/type :fsv/Module})
    (aa/add @triples {:rdf/about :bhp.module/Crown :rdf/type :fsv/Module})
    (aa/add @triples {:rdf/about :bhp.module/Mortality :rdf/type :fsv/Module})

    (aa/add @triples {:rdf/about :bhp/SubmoduleProperty :owl/subClassOf :fsv/Concept})
    (aa/add @triples {:rdf/about :bhp/InputOutputType :owl/subClassOf :bhp/SubmoduleProperty})

    (aa/add @triples {:rdf/about :bhp/InputOutputType :owl/subClassOf :bhp/SubmoduleProperty})

    (aa/add @triples {:rdf/about :bhp/IOType :owl/subClassOf :fsv/Concept})

    (aa/add @triples {:rdf/about :bhp/hasIO :rdf/type :owl/ObjectProperty})

    (aa/add @triples {:rdf/about :bhp.submodule.output/Fire :fsv/hasModule :bhp.module/Contain :rdf/type :fsv/Submodule})
    (aa/add @triples {:rdf/about :bhp.submodule.input/Fire :fsv/hasModule :bhp.module/Contain :rdf/type :fsv/Submodule})
    (aa/add @triples {:rdf/about :bhp.submodule.input/Suppression :fsv/hasModule :bhp.module/Contain :rdf/type :fsv/Submodule})

    (aa/add @triples {:rdf/about :bhp.group/Resources :fsv/hasModule :bhp.module/Contain :rdf/type :fsv/Submodule})

    (q/run @triples '[:bgp
                        [:bhp.module/Contain :fsv/hasChild ?o]])

    #_(q/run @triples '[:bgp
                        [:bhp.module/Contain :fsv/hasSubmodule ?o]])
    #_(q/run @triples '[:bgp [:fsv/Module ?a ?o]])))
