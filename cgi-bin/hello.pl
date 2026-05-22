#!/usr/bin/perl
use strict;
use warnings;
print "Content-Type: text/plain\r\n\r\n";
print "Hello from Perl CGI (bonus)\n";
print "PATH_INFO=$ENV{PATH_INFO}\n";
