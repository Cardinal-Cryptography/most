const gulp = require('gulp');
const shell = require('gulp-shell')

gulp.task('compile', shell.task('truffle compile'))

gulp.task('watch', function () {
  gulp.watch("contracts/*.sol", gulp.series('compile'));
});

gulp.task('default', gulp.series('watch'));
