import org.l2jmobius.commons.network.Buffer;
import org.l2jmobius.gameserver.network.Encryption;

/**
 * Dumps golden vectors from the real Java {@link Encryption} (the game client's
 * rolling XOR cipher) as JSON, for the Rust port's parity tests
 * ({@code crates/gameserver/tests/cipher_vectors.rs}). Run once; the output is
 * committed to the Rust repo.
 *
 * It drives one Encryption instance through a scripted sequence that exercises
 * the first-call pass-through, encrypt and decrypt, independent per-direction
 * key rolling, and varying packet sizes. The Rust test replays the identical
 * sequence and asserts identical bytes.
 */
public class GameCipherVectors
{
	/** Little-endian byte[] Buffer, mirroring Async-mmocore's LE ByteBuffers. */
	static class ByteArrayBuffer implements Buffer
	{
		private final byte[] data;

		ByteArrayBuffer(byte[] data)
		{
			this.data = data;
		}

		public byte readByte(int index)
		{
			return data[index];
		}

		public void writeByte(int index, byte value)
		{
			data[index] = value;
		}

		public short readShort(int index)
		{
			return (short) ((data[index] & 0xff) | ((data[index + 1] & 0xff) << 8));
		}

		public void writeShort(int index, short value)
		{
			data[index] = (byte) value;
			data[index + 1] = (byte) (value >> 8);
		}

		public int readInt(int index)
		{
			return (data[index] & 0xff) | ((data[index + 1] & 0xff) << 8) | ((data[index + 2] & 0xff) << 16) | ((data[index + 3] & 0xff) << 24);
		}

		public void writeInt(int index, int value)
		{
			data[index] = (byte) value;
			data[index + 1] = (byte) (value >> 8);
			data[index + 2] = (byte) (value >> 16);
			data[index + 3] = (byte) (value >> 24);
		}

		public int limit()
		{
			return data.length;
		}

		public void limit(int newLimit)
		{
		}
	}

	static String hex(byte[] b)
	{
		final StringBuilder sb = new StringBuilder();
		for (byte x : b)
		{
			sb.append(String.format("%02x", x & 0xff));
		}
		return sb.toString();
	}

	/** A deterministic byte pattern of the given length. */
	static byte[] pattern(int len, int seed)
	{
		final byte[] b = new byte[len];
		for (int i = 0; i < len; i++)
		{
			b[i] = (byte) ((i * 31) + seed);
		}
		return b;
	}

	public static void main(String[] args) throws Exception
	{
		// [8 random | 8 static] — arbitrary but fixed random half for reproducibility.
		final byte[] key = new byte[]
		{
			(byte) 0x11, (byte) 0x22, (byte) 0x33, (byte) 0x44, (byte) 0x55, (byte) 0x66, (byte) 0x77, (byte) 0x88,
			(byte) 0xc8, (byte) 0x27, (byte) 0x93, (byte) 0x01, (byte) 0xa1, (byte) 0x6c, (byte) 0x31, (byte) 0x97
		};

		final Encryption e = new Encryption();
		e.setKey(key);

		// Scripted op sequence: (op, input). "e" = encrypt, "d" = decrypt.
		final String[] ops =
		{
			"e", "e", "d", "e", "d", "e"
		};
		final byte[][] inputs =
		{
			pattern(10, 1),  // KeyPacket-sized first call (pass-through)
			pattern(16, 2),  // encrypt, out_key shift by 16
			pattern(7, 3),   // decrypt, in_key shift by 7
			pattern(33, 4),  // encrypt, out_key shift again
			pattern(5, 5),   // decrypt, in_key shift again
			pattern(64, 6)   // encrypt, larger
		};

		final StringBuilder json = new StringBuilder();
		json.append("{\n");
		json.append("  \"key\": \"").append(hex(key)).append("\",\n");
		json.append("  \"steps\": [\n");
		for (int i = 0; i < ops.length; i++)
		{
			final byte[] work = inputs[i].clone();
			final ByteArrayBuffer buf = new ByteArrayBuffer(work);
			if (ops[i].equals("e"))
			{
				e.encrypt(buf, 0, work.length);
			}
			else
			{
				e.decrypt(buf, 0, work.length);
			}
			json.append("    { \"op\": \"").append(ops[i]).append("\", \"in\": \"").append(hex(inputs[i])).append("\", \"out\": \"").append(hex(work)).append("\" }");
			json.append(i == ops.length - 1 ? "\n" : ",\n");
		}
		json.append("  ]\n");
		json.append("}\n");
		System.out.print(json);
	}
}
