# frozen_string_literal: true

require 'socket'
require 'open-uri'

module Archival
  class HelperServer
    attr_reader :page

    def initialize(port, build_dir)
      @port = port
      @build_dir = build_dir
      @helper_dir = File.expand_path(File.join(File.dirname(__FILE__),
                                               '../../helper'))
    end

    def start
      server = TCPServer.new @port
      loop do
        Thread.start(server.accept) do |client|
          req = ''
          method = nil
          path = nil
          while (line = client.gets) && (line != "\r\n")
            unless method
              req_info = line.split
              method = req_info[0]
              path = req_info[1]
            end
            req += line
          end
          client.close unless req
          handle_request(client, req, method, path)
        end
      end
    end

    def refresh_client
      ws_sendmessage('refresh')
    end

    private

    MAGIC_GUID = '258EAFA5-E914-47DA-95CA-C5AB0DC85B11'

    def handle_request(client, req, method, path)
      if method == 'GET' && path.start_with?('/js/archival-helper.js')
        # For this special file, serve it from the helper dir
        http_response(client, type: 'application/javascript') do
          serve_static(client, path, @helper_dir)
        end
        client.close
      elsif (matches = req.match(/^Sec-WebSocket-Key: (\S+)/))
        websocket_key = matches[1]
        # puts "Websocket handshake detected with key: #{websocket_key}"
        connect_socket(client, websocket_key)
      elsif method == 'GET'
        # For static paths, just serve the files they refer to.
        # TODO: mime type should be inferred from file type
        http_response(client, type: 'application/javascript') do
          serve_static(client, path)
        end
        client.close
      else
        client.close
      end
    end

    def connect_socket(client, websocket_key)
      @socket = client
      response_key = Digest::SHA1.base64digest([websocket_key,
                                                MAGIC_GUID].join)
      #   puts "Responding to handshake with key: #{response_key}"

      @socket.write "HTTP/1.1 101 Switching Protocols\r\n"
      @socket.write "Upgrade: websocket\r\n"
      @socket.write "Connection: Upgrade\r\n"
      @socket.write "Sec-WebSocket-Accept: #{response_key}\r\n"
      @socket.write "\r\n"

      #   puts 'Handshake completed.'
      ws_loop
    end

    def ws_loop
      loop do
        msg = ws_getmessage
        next unless msg

        if msg == 'connected'
          ws_sendmessage('ready')
        elsif msg.start_with?('page:')
          page_path = Pathname.new(msg.sub(/^page:/, ''))
          @page = page_path.relative_path_from(@build_dir)
          ws_sendmessage('ok')
        end
      end
    end

    def validate_ws_message
      first_byte = @socket.getbyte
      return unless first_byte

      fin = first_byte & 0b10000000
      opcode = first_byte & 0b00001111

      # Our server only supports single-frame, text messages.
      # Raise an exception if the client tries to send anything else.
      raise 'Archival dev server does not support continuations' unless fin
      # Some browsers send this regardless, so ignore it to keep the noise down.
      return unless opcode == 1

      second_byte = @socket.getbyte
      is_masked = second_byte & 0b10000000
      payload_size = second_byte & 0b01111111

      raise 'frame masked incorrectly' unless is_masked
      raise 'payload must be < 126 bytes in length' unless payload_size < 126

      payload_size
    end

    def ws_getmessage
      payload_size = validate_ws_message
      return unless payload_size

      #   warn "Payload size: #{payload_size} bytes"

      mask = 4.times.map { @socket.getbyte }
      #   warn "Got mask: #{mask.inspect}"

      data = payload_size.times.map { @socket.getbyte }
      #   warn "Got masked data: #{data.inspect}"

      unmasked_data = data.each_with_index.map do |byte, i|
        byte ^ mask[i % 4]
      end
      #   warn "Unmasked the data: #{unmasked_data.inspect}"

      unmasked_data.pack('C*').force_encoding('utf-8')
    end

    def ws_sendmessage(message)
      return unless @socket

      output = [0b10000001, message.size, message]
      @socket.write output.pack("CCA#{message.size}")
    end

    def serve_static(client, path, base = @build_dir)
      buffer = File.open(File.join(base, path)).read
      buffer.sub! '$PORT', @port.to_s
      client.print buffer
    end

    def http_response(client, config)
      status = config[:status] ||= 200
      type = config[:type] ||= 'text/html'
      client.print "HTTP/1.1 #{status}\r\n"
      client.print "Content-Type: #{type}\r\n"
      client.print "Access-Control-Allow-Origin: *\r\n"
      client.print "\r\n"
      yield
    end
  end
end
