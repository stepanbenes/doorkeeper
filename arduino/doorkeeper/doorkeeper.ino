#define LED_PIN 13
#define BUTTON_PIN 3
#define NOISE_THRESHOLD_PIN 2
#define NOISE_LEVEL_PIN A0
#define BUZZER_PIN 8
#define RX_PIN 11
#define TX_PIN 10

#define LOOP_DELAY_TIME 10

#define BUTTON_DEBOUNCE_TIME 100
#define BUTTON_HOLD_TIME 1500

#define SAMPLES 128             //SAMPLES-pt FFT. Must be a base 2 number. Max 128 for Arduino Uno.
#define SAMPLING_FREQUENCY 2048 //Ts = Based on Nyquist, must be 2 times the highest expected frequency.
#define MAX_AMPLITUDE (1024 / 2)
#define DEFAULT_VOLUME_THRESHOLD 5 // %

#define BUZZER_DEFAULT_DURATION 2000 // ms
#define BUZZER_MAX_DURATION 5000 // ms

// ===========================

#include <avr/wdt.h>
#include "arduinoFFT.h"
#include <SoftwareSerial.h>

// ===========================
SoftwareSerial bluetooth(TX_PIN, RX_PIN);
// ===========================
volatile unsigned long last_button_interrupt_time;
bool is_button_down;
bool is_button_hold;
// ===========================
unsigned long buzzer_start_time;
unsigned long buzzer_duration;
// ===========================

// ===========================
arduinoFFT FFT = arduinoFFT();
 
unsigned int samplingPeriod;
 
double vReal[SAMPLES]; //create vector of size SAMPLES to hold real values
double vImag[SAMPLES]; //create vector of size SAMPLES to hold imaginary values

unsigned int volume_threshold;
unsigned long last_noise_detected_time;
bool is_noise_detected;

double frequency_accumulator;
double min_peak_frequency, max_peak_frequency;
unsigned int frequency_counter;
double max_sound_level;
// ===========================

void setup()
{
  last_button_interrupt_time = 0;
  last_noise_detected_time = 0;
  is_noise_detected = false;
  frequency_accumulator = 0.0;
  min_peak_frequency = max_peak_frequency = 0.0;
  frequency_counter = 0;
  max_sound_level = 0.0;
  buzzer_duration = BUZZER_DEFAULT_DURATION;
  volume_threshold = DEFAULT_VOLUME_THRESHOLD;
  samplingPeriod = round(1000000*(1.0/SAMPLING_FREQUENCY)); //Period in microseconds
  
  Serial.begin(9600);
  bluetooth.begin(9600);

  // nastavení LED diody jako výstupní a její vypnutí
  pinMode(LED_PIN, OUTPUT);
  digitalWrite(LED_PIN, LOW);

  // button
  pinMode(BUTTON_PIN, INPUT_PULLUP);
  attachInterrupt(digitalPinToInterrupt(BUTTON_PIN), button_change_interrupt, CHANGE);
  is_button_down = !digitalRead(BUTTON_PIN); // Pull-up resistors invert the logic, so true == off, false == on
  is_button_hold = false;

  // noise
  pinMode(NOISE_LEVEL_PIN, INPUT);

  // print default values
  bluetooth.println("hello");
  bluetooth.println("volume-threshold:" + String(volume_threshold));
  bluetooth.println("buzzer-duration:" + String(buzzer_duration));
}

void loop()
{
  // handle buzzer
  if (digitalRead(BUZZER_PIN) == LOW && ((millis() - buzzer_start_time) >= buzzer_duration)) { // overflow should not matter if calculating in unsigned integer arithmetics
    digitalWrite(BUZZER_PIN, HIGH); // turn off buzzer
    bluetooth.println("buzzer-off");
  }

  // analyze sound
  {
    unsigned long loop_begin_time = millis();
    double peak_frequency;
    double volume_max, volume_average, volume_rms, volume_dB;
    
    analyzeSound(&peak_frequency, &volume_max, &volume_average, &volume_rms, &volume_dB);
    
    if (volume_max >= volume_threshold) {
      if (!is_noise_detected) {
        is_noise_detected = true;
        last_noise_detected_time = loop_begin_time;
        frequency_accumulator = 0.0;
        frequency_counter = 0;
        min_peak_frequency = max_peak_frequency = peak_frequency;
        max_sound_level = 0.0;
      }
      frequency_accumulator += peak_frequency;
      frequency_counter++;
      min_peak_frequency = min(min_peak_frequency, peak_frequency);
      max_peak_frequency = max(max_peak_frequency, peak_frequency);
      max_sound_level = max(max_sound_level, volume_max);
      //Serial.println("peak frequency: " + String(peak_frequency) + "; max volume: " + String(volume_max) + "; avg volume: " + String(volume_average) + "; rms volume: " + String(volume_rms) + "; db: " + String(volume_dB));
    }
    else {
      if (is_noise_detected) {
        is_noise_detected = false;
        unsigned long noise_duration = millis() - last_noise_detected_time;
        double average_peak_frequency = frequency_accumulator / frequency_counter;
        double max_deviation = max(average_peak_frequency - min_peak_frequency, max_peak_frequency - average_peak_frequency);
        int average_peak_frequency_int = (int)round(average_peak_frequency);
        int max_deviation_int = (int)round(max_deviation);
        int max_sound_level_int = (int)round(max_sound_level);
        bluetooth.println("noise:" + String(noise_duration) + ":" + String(max_sound_level_int));
        bluetooth.println("freq:" + String(average_peak_frequency_int) + ":" + String(max_deviation_int));
      }
    }
  }

  // handle button state
  {
    unsigned long time_from_last_change = millis() - last_button_interrupt_time;
    if (time_from_last_change >= (unsigned long)BUTTON_DEBOUNCE_TIME) {
      bool current_is_button_down = !digitalRead(BUTTON_PIN);
      if (current_is_button_down != is_button_down) {
        is_button_down = current_is_button_down;
        is_button_hold = false;
        if (is_button_down) {
          bluetooth.println("button-down");
        }
        else {
          bluetooth.println("button-up");
        }
      }
      else if (is_button_down && !is_button_hold && (time_from_last_change >= (unsigned long)BUTTON_HOLD_TIME)) {
        is_button_hold = true;
        bluetooth.println("button-hold");
      }
    }
  }
  
  // handle communication
  if (bluetooth.available() > 0) {
    byte bluetoothData = bluetooth.read();
    switch (bluetoothData) {
      case '0':
        digitalWrite(LED_PIN, LOW);
        bluetooth.println("led-off");
        break;
      case '1':
        digitalWrite(LED_PIN, HIGH);
        bluetooth.println("led-on");
        break;
      case 't':
        bluetooth.print("uptime:" + String(millis()));
        break;
      case 'v':
        {
          long value = bluetooth.parseInt();
          if (value > 0) {
            volume_threshold = (int)value;
            is_noise_detected = false; // reset noise detection
          }
          bluetooth.println("volume-threshold:" + String(volume_threshold));
        }
        break;
      case 'b':
        {
          long value = bluetooth.parseInt();
          if (value > 0) {
            buzzer_duration = min((unsigned long)(value), (unsigned long)BUZZER_MAX_DURATION);
          }
          bluetooth.println("buzzer-duration:" + String(buzzer_duration));
        }
        break;
      case 'x':
        {
          buzzer_start_time = millis(); // remember buzzer start time
          digitalWrite(BUZZER_PIN, LOW); // turn on buzzer
          bluetooth.println("buzzer-on");
        }
        break;
      case 'r':
        bluetooth.println("rebooting...");
        reboot();
        break;
      case '\r':
      case '\n':
        // ignore
        break;
      default:
        // report invalid command
        bluetooth.print("invalid-command:" + String(bluetoothData) + "," + String((char)bluetoothData));
        break;
    }
  }

  delay(LOOP_DELAY_TIME);
}

void button_change_interrupt()
{
  last_button_interrupt_time = millis();
}

void reboot()
{
  wdt_disable();
  wdt_enable(WDTO_15MS);
  while (true) {
    // wait for reboot
  }
}

// calculate volume level of the signal
void measureVolume(double *samples, double *volume_max, double *volume_average, double *volume_RMS, double *volume_dB)
{
  double soundVolAvg = 0, soundVolMax = 0, soundVolRMS = 0;
  //cli();  // UDRE interrupt slows this way down on arduino1.0
  for (int i = 0; i < SAMPLES; i++)
  {
    int k = samples[i];
    int amp = abs(k - MAX_AMPLITUDE);
    soundVolMax = max(soundVolMax, amp);
    soundVolAvg += amp;
    soundVolRMS += ((long)amp*amp);
  }
  soundVolAvg /= SAMPLES;
  soundVolRMS /= SAMPLES;
  float soundVolRMSflt = sqrt(soundVolRMS);
  //sei();

  float dB = 20.0 * log10(soundVolRMSflt / MAX_AMPLITUDE);

  // convert from 0 to 100
  soundVolAvg = 100.0 * soundVolAvg / MAX_AMPLITUDE; 
  soundVolMax = 100.0 * soundVolMax / MAX_AMPLITUDE; 
  soundVolRMSflt = 100.0 * soundVolRMSflt / MAX_AMPLITUDE;
  soundVolRMS = 10.0 * soundVolRMSflt / 7.0; // RMS to estimate peak (RMS is 0.7 of the peak in sin)

  *volume_average = soundVolAvg;
  *volume_max = soundVolMax;
  *volume_RMS = soundVolRMS;
  *volume_dB = dB;
}

void analyzeSound(double* peak_frequency, double* volume_max, double* volume_average, double* volume_rms, double* volume_dB)
{
    int soundVolMax = 0;
    /*Sample SAMPLES times*/
    for(int i=0; i < SAMPLES; i++)
    {
        unsigned long microSeconds = micros(); // Returns the number of microseconds since the Arduino board began running the current script. 

        int k = analogRead(NOISE_LEVEL_PIN);
        int amp = abs(k - MAX_AMPLITUDE);
        soundVolMax = max(soundVolMax, amp);
     
        vReal[i] = k; //Reads the value from analog pin 0 (A0), quantize it and save it as a real term.
        vImag[i] = 0; //Makes imaginary term 0 always

        /*remaining wait time between samples if necessary*/
        while(micros() < (microSeconds + samplingPeriod)) {
          //do nothing
        }
    }

    measureVolume(vReal, volume_max, volume_average, volume_rms, volume_dB);

    /*Perform FFT on samples*/
    FFT.Windowing(vReal, SAMPLES, FFT_WIN_TYP_HAMMING, FFT_FORWARD);
    FFT.Compute(vReal, vImag, SAMPLES, FFT_FORWARD);
    FFT.ComplexToMagnitude(vReal, vImag, SAMPLES);

    /*Find most dominant frequency*/
    double peak = FFT.MajorPeak(vReal, SAMPLES, SAMPLING_FREQUENCY);
    *peak_frequency = peak;
}
