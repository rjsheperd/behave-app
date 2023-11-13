module.exports = {
    globDirectory: 'resources/public/',
    globPatterns: ['**/*.{html,css,js,jpg,jpeg,png,msgpack,csv,woff2}'],
    maximumFileSizeToCacheInBytes: 5000000,
    swSrc: 'src/js/sw.js',
    swDest: 'dist/js/sw.js'
};
