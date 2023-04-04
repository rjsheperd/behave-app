ifndef WEBIDL
	WEBIDL := $(shell brew --prefix emscripten)/libexec/tools/webidl_binder
endif

all: install

bind:
	${WEBIDL} include/idl/behave.idl include/js/glue
	mv include/js/glue.cpp include/cpp/emscripten/glue.cpp

compile: bind
	emcmake cmake -B build
	cmake --build build

install: compile
#	cp build/behave.js ../projects/behave/resources/public/js
	cp build/behave-min.* ../projects/behave/resources/public/js

# Only compile the tests (testBehave, testContain, testSurface)
compile-test:
	cmake -DTEST_DEBUG=OFF -DTEST_BEHAVE=ON -DTEST_CONTAIN=ON -DTEST_SURFACE=ON -DBEHAVE_WASM=OFF -B test-build
	cmake --build test-build

# Run the tests (testBehave, testContain, testSurface)
run-test: compile-test
	echo "${printf -v date '%(%Y-%m-%d %H:%M:%S)T\n' -1}" > test-build/outputs.txt
	echo "##### BEHAVE TESTS #####" >> test-build/outputs.txt
	./test-build/testBehave >> test-build/outputs.txt
	echo "##### CONTAIN TESTS #####" >> test-build/outputs.txt
	./test-build/testContain test/csv/contain.csv >> test-build/outputs.txt
	echo "##### SURFACE TESTS #####" >> test-build/outputs.txt
	./test-build/testSurface test/csv/surface.csv >> test-build/outputs.txt

clean:
	rm -rf {build,test-build}

ctags:
	ctags -e -R --exclude=.git --exclude=build --exclude .ccls-cache .
