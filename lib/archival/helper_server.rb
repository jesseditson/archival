# frozen_string_literal: true

require 'socket'
require 'open-uri'

module Archival
  class HelperServer
    attr_reader :socket

    def initialize(port)
      @port = port
      @helper_dir = File.expand_path(File.join(File.dirname(__FILE__),
                                               '../../helper'))
    end

    def start
      server = TCPServer.new @port
      loop do
        Thread.start(server.accept) do |client|
          req = client.gets
          client.close unless req
          req = req.split
          method = req[0]
          path = req[1]
          handle_request(client, method, path)
        end
      end
    end

    private

    def handle_request(client, _method, path)
      if path.start_with? '/js/'
        # For static paths, just serve the files they refer to.
        http_response(client, type: 'application/javascript') do
          serve_static(client, path)
        end
        client.close
      else
        # A root request connects a socket
        @socket = client
      end
    end

    def connect_socket(client); end

    def serve_static(client, path)
      buffer = open(File.join(@helper_dir, path)).read
      buffer.sub! '$PORT', @port.to_s
      client.print buffer
    end

    def http_response(client, config)
      status = config[:status] ||= 200
      type = config[:type] ||= 'text/html'
      client.print "HTTP/1.1 #{status}\r\n"
      client.print "Content-Type: #{type}\r\n"
      client.print "\r\n"
      yield
    end
  end
end
